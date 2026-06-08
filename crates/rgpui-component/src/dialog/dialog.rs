use std::{rc::Rc, sync::LazyLock, time::Duration};

use rgpui::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, BoxShadow, ClickEvent, Edges,
    FocusHandle, Hsla, InteractiveElement, IntoElement, KeyBinding, MouseButton, ParentElement,
    Pixels, Point, RenderOnce, SharedString, StyleRefinement, Styled, Window, WindowControlArea,
    actions, anchored, div, hsla, point, prelude::FluentBuilder, px,
};
use rust_i18n::t;

use crate::{
    ActiveTheme as _, FocusTrapElement as _, IconName, Root, Sizable as _, StyledExt,
    TITLE_BAR_HEIGHT, WindowExt as _,
    animation::cubic_bezier,
    button::{Button, ButtonVariant, ButtonVariants as _},
    dialog::{DialogContent, DialogTitle},
    scroll::ScrollableElement as _,
    v_flex,
};

/// 对话框动画持续时间（0.25 秒）
pub static ANIMATION_DURATION: LazyLock<Duration> = LazyLock::new(|| Duration::from_secs_f64(0.25));

/// 对话框键盘上下文标识
const CONTEXT: &str = "Dialog";

// 注册对话框相关动作
actions!(dialog, [CancelDialog, ConfirmDialog]);

/// 初始化对话框模块
///
/// 绑定 Escape 键（取消对话框）和 Enter 键（确认对话框）的快捷键。
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CancelDialog, Some(CONTEXT)),
        KeyBinding::new("enter", ConfirmDialog, Some(CONTEXT)),
    ]);
}

/// 对话框按钮属性配置
///
/// 包含确定按钮和取消按钮的文本、样式变体、回调函数等配置。
#[derive(Clone)]
pub struct DialogButtonProps {
    /// 确定按钮文本
    pub(crate) ok_text: Option<SharedString>,
    /// 确定按钮样式变体
    pub(crate) ok_variant: ButtonVariant,
    /// 取消按钮文本
    pub(crate) cancel_text: Option<SharedString>,
    /// 取消按钮样式变体
    pub(crate) cancel_variant: ButtonVariant,
    /// 是否显示取消按钮
    pub(crate) show_cancel: bool,
    /// 确定按钮点击回调
    pub(crate) on_ok: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static>,
    /// 取消按钮点击回调
    pub(crate) on_cancel: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static>,
    /// 对话框关闭回调
    pub(crate) on_close: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
}

impl Default for DialogButtonProps {
    /// 创建默认的按钮属性配置
    ///
    /// 默认只显示确定按钮，不显示取消按钮。
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
    /// 设置确定按钮的文本，默认为 "确定"
    pub fn ok_text(mut self, ok_text: impl Into<SharedString>) -> Self {
        self.ok_text = Some(ok_text.into());
        self
    }

    /// 设置确定按钮的样式变体，默认为 `ButtonVariant::Primary`
    pub fn ok_variant(mut self, ok_variant: ButtonVariant) -> Self {
        self.ok_variant = ok_variant;
        self
    }

    /// 设置取消按钮的文本，默认为 "取消"
    pub fn cancel_text(mut self, cancel_text: impl Into<SharedString>) -> Self {
        self.cancel_text = Some(cancel_text.into());
        self
    }

    /// 设置取消按钮的样式变体，默认为 `ButtonVariant::default()`
    pub fn cancel_variant(mut self, cancel_variant: ButtonVariant) -> Self {
        self.cancel_variant = cancel_variant;
        self
    }

    /// 设置是否显示取消按钮，默认为 `false`
    pub fn show_cancel(mut self, show_cancel: bool) -> Self {
        self.show_cancel = show_cancel;
        self
    }

    /// 设置确定按钮点击回调
    ///
    /// 回调返回 `true` 时关闭对话框，返回 `false` 时对话框保持打开。
    pub fn on_ok(
        mut self,
        on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.on_ok = Rc::new(on_ok);
        self
    }

    /// 设置取消按钮点击回调
    ///
    /// 回调返回 `true` 时关闭对话框，返回 `false` 时对话框保持打开。
    pub fn on_cancel(
        mut self,
        on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.on_cancel = Rc::new(on_cancel);
        self
    }

    /// 渲染确定按钮
    ///
    /// 点击后执行 on_ok 回调，如果返回 true 则关闭对话框并执行 on_close 回调。
    pub(crate) fn render_ok(&self, _: &mut Window, _: &mut App) -> AnyElement {
        let on_ok = self.on_ok.clone();
        let on_close = self.on_close.clone();

        let ok_text = self
            .ok_text
            .clone()
            .unwrap_or_else(|| t!("Dialog.ok").into());
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

    /// 渲染取消按钮
    ///
    /// 点击后执行 on_cancel 回调，如果返回 true 则关闭对话框并执行 on_close 回调。
    pub(crate) fn render_cancel(&self, _: &mut Window, _: &mut App) -> AnyElement {
        let on_cancel = self.on_cancel.clone();
        let on_close = self.on_close.clone();
        let cancel_text = self
            .cancel_text
            .clone()
            .unwrap_or_else(|| t!("Dialog.cancel").into());
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

/// 内容构建器函数类型
type ContentBuilderFn = Rc<dyn Fn(DialogContent, &mut Window, &mut App) -> DialogContent + 'static>;

/// 对话框配置属性
#[derive(Clone)]
pub(crate) struct DialogProps {
    /// 对话框宽度
    width: Pixels,
    /// 对话框最大宽度
    max_width: Option<Pixels>,
    /// 对话框顶部偏移量
    margin_top: Option<Pixels>,
    /// 是否显示关闭按钮
    close_button: bool,

    /// 是否显示遮罩层
    overlay: bool,
    /// 是否支持点击遮罩层关闭对话框
    overlay_closable: bool,
    /// 遮罩层是否可见
    pub(crate) overlay_visible: bool,
    /// 是否支持键盘 ESC 键关闭对话框
    keyboard: bool,
}

impl Default for DialogProps {
    /// 创建默认的对话框配置
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

/// 模态对话框组件
///
/// 用于在对话框中显示内容的模态窗口，支持遮罩层、动画、键盘快捷键等特性。
#[derive(IntoElement)]
pub struct Dialog {
    /// 样式 refinement
    pub(crate) style: StyleRefinement,
    /// 子元素列表
    children: Vec<AnyElement>,
    /// 触发器元素
    trigger: Option<AnyElement>,
    /// 标题元素
    title: Option<AnyElement>,
    /// 头部元素
    pub(crate) header: Option<AnyElement>,
    /// 底部元素
    pub(crate) footer: Option<AnyElement>,
    /// 内容构建器
    pub(crate) content_builder: Option<ContentBuilderFn>,
    /// 对话框配置属性
    pub(crate) props: DialogProps,

    /// 按钮属性配置
    button_props: DialogButtonProps,

    /// 焦点句柄，在对话框打开时创建
    pub(crate) focus_handle: FocusHandle,
    /// 对话框层级索引，用于支持多层对话框叠加
    pub(crate) layer_ix: usize,
}

/// 获取遮罩层颜色
///
/// 如果 `overlay` 为 false，则返回完全透明的颜色。
pub(crate) fn overlay_color(overlay: bool, cx: &App) -> Hsla {
    if !overlay {
        return hsla(0., 0., 0., 0.);
    }

    cx.theme().overlay
}

impl Dialog {
    /// 创建新的对话框
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
        }
    }

    /// 设置触发器元素
    ///
    /// 设置触发器后，对话框将渲染为一个可点击的元素，点击后打开对话框。
    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    /// 设置对话框内容构建器
    pub fn content<F>(mut self, builder: F) -> Self
    where
        F: Fn(DialogContent, &mut Window, &mut App) -> DialogContent + 'static,
    {
        self.content_builder = Some(Rc::new(builder));
        self
    }

    /// 设置对话框标题
    pub fn title(mut self, title: impl IntoElement) -> Self {
        self.title = Some(title.into_any_element());
        self
    }

    /// 设置对话框头部
    ///
    /// 头部位于对话框顶部，通常包含标题和描述。
    pub(crate) fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// 设置对话框底部区域
    ///
    /// 底部区域位于对话框最下方，通常用于放置操作按钮。
    /// 设置底部区域后，`button_props` 将被忽略，需要自行渲染操作按钮。
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// 设置按钮属性配置
    pub fn button_props(mut self, button_props: DialogButtonProps) -> Self {
        self.button_props = button_props;
        self
    }

    /// 设置对话框关闭时的回调
    ///
    /// 在 `on_ok` 或 `on_cancel` 回调之后调用。
    pub fn on_close(
        mut self,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.button_props.on_close = Rc::new(on_close);
        self
    }

    /// 设置确定按钮点击回调
    ///
    /// 回调返回 `true` 时关闭对话框，返回 `false` 时对话框保持打开。
    pub fn on_ok(
        mut self,
        on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.button_props = self.button_props.on_ok(on_ok);
        self
    }

    /// 设置取消按钮点击回调
    ///
    /// 回调返回 `true` 时关闭对话框，返回 `false` 时对话框保持打开。
    pub fn on_cancel(
        mut self,
        on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.button_props = self.button_props.on_cancel(on_cancel);
        self
    }

    /// 设置是否显示关闭按钮，默认为 `true`
    pub fn close_button(mut self, close_button: bool) -> Self {
        self.props.close_button = close_button;
        self
    }

    /// 设置对话框顶部偏移量，默认为视口高度的 1/10
    pub fn margin_top(mut self, margin_top: impl Into<Pixels>) -> Self {
        self.props.margin_top = Some(margin_top.into());
        self
    }

    /// 设置对话框宽度，默认为 448px
    pub fn w(mut self, width: impl Into<Pixels>) -> Self {
        self.props.width = width.into();
        self
    }

    /// 设置对话框宽度，默认为 448px
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.props.width = width.into();
        self
    }

    /// 设置对话框最大宽度，默认为 `None`（无限制）
    pub fn max_w(mut self, max_width: impl Into<Pixels>) -> Self {
        self.props.max_width = Some(max_width.into());
        self
    }

    /// 设置是否显示遮罩层，默认为 `true`
    pub fn overlay(mut self, overlay: bool) -> Self {
        self.props.overlay = overlay;
        self
    }

    /// 设置是否支持点击遮罩层关闭对话框，默认为 `true`
    pub fn overlay_closable(mut self, overlay_closable: bool) -> Self {
        self.props.overlay_closable = overlay_closable;
        self
    }

    /// 设置是否支持键盘 ESC 键关闭对话框，默认为 `true`
    pub fn keyboard(mut self, keyboard: bool) -> Self {
        self.props.keyboard = keyboard;
        self
    }

    /// 判断是否显示遮罩层
    pub(crate) fn has_overlay(&self) -> bool {
        self.props.overlay
    }

    /// 应用对话框配置属性
    pub(crate) fn with_props(mut self, props: DialogProps) -> Self {
        self.props = props;
        self
    }

    /// 延迟关闭对话框
    ///
    /// 通过 Root 组件延迟关闭当前活动的对话框。
    fn defer_close_dialog(window: &mut Window, cx: &mut App) {
        Root::update(window, cx, |root, window, cx| {
            root.defer_close_dialog(window, cx);
        });
    }
}

impl ParentElement for Dialog {
    /// 向对话框中添加子元素
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Dialog {
    /// 获取可修改的样式引用
    fn style(&mut self) -> &mut rgpui::StyleRefinement {
        &mut self.style
    }
}

impl Dialog {
    /// 渲染触发器模式的对话框
    ///
    /// 点击触发器元素时打开对话框。
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
    /// 渲染对话框
    ///
    /// 如果设置了触发器，渲染触发器模式；否则直接渲染对话框内容。
    /// 支持遮罩层、动画、键盘快捷键、多层叠加等特性。
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if let Some(trigger) = self.trigger.take() {
            return self.render_trigger(trigger, window, cx);
        }

        let layer_ix = self.layer_ix;
        let on_close = self.button_props.on_close.clone();
        let on_ok = self.button_props.on_ok.clone();
        let on_cancel = self.button_props.on_cancel.clone();

        // 计算窗口边距和视口大小
        let window_paddings = crate::window_border::window_paddings(window);
        let view_size = window.viewport_size()
            - rgpui::size(
                window_paddings.left + window_paddings.right,
                window_paddings.top + window_paddings.bottom,
            );
        let bounds = Bounds {
            origin: Point::default(),
            size: view_size,
        };
        // 根据层级索引计算偏移量，支持多层对话框叠加
        let offset_top = px(layer_ix as f32 * 16.);
        let y = self.props.margin_top.unwrap_or(view_size.height / 10.) + offset_top;
        let x = bounds.center().x - self.props.width / 2.;

        // 获取文本样式基准值
        let base_size = window.text_style().font_size;
        let rem_size = window.rem_size();

        // 解析内边距
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

        // 创建动画（滑入 + 淡入效果）
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
                    // 遮罩层背景色
                    .when(self.props.overlay_visible, |this| {
                        this.bg(overlay_color(self.props.overlay, cx))
                    })
                    // 遮罩层点击关闭逻辑
                    .when(self.props.overlay, |this| {
                        // 只有最后一个对话框拥有"点击遮罩层关闭"事件
                        if (self.layer_ix + 1) != Root::read(window, cx).active_dialogs.len() {
                            return this;
                        }

                        this.window_control_area(WindowControlArea::Drag)
                            .on_any_mouse_down({
                                let on_cancel = on_cancel.clone();
                                let on_close = on_close.clone();
                                move |event, window, cx| {
                                    // 忽略标题栏区域的点击事件
                                    if event.position.y < TITLE_BAR_HEIGHT {
                                        return;
                                    }

                                    cx.stop_propagation();
                                    // 点击遮罩层时关闭对话框
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
                    // 对话框主体内容
                    .child(
                        v_flex()
                            .id(layer_ix)
                            .track_focus(&self.focus_handle)
                            .focus_trap(format!("dialog-{}", layer_ix), &self.focus_handle)
                            .bg(cx.theme().background)
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
                            // 键盘快捷键支持（ESC 取消、Enter 确认）
                            .when(self.props.keyboard, |this| {
                                this.on_action({
                                    let on_cancel = on_cancel.clone();
                                    let on_close = on_close.clone();
                                    move |_: &CancelDialog, window, cx| {
                                        // FIXME: 部分对话框没有 focus_handle，ESC 键可能不生效
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
                            // 高优先级样式，不可被覆盖
                            .absolute()
                            .occlude()
                            .relative()
                            .left(x)
                            .top(y)
                            .w(self.props.width)
                            .when_some(self.props.max_width, |this, w| this.max_w(w))
                            // 对话框内部布局：头部 → 标题 → 内容 → 子元素
                            .child(
                                v_flex()
                                    .flex_1()
                                    .overflow_hidden()
                                    .gap_y_2()
                                    // 头部区域
                                    .when_some(self.header, |this, header| {
                                        this.child(
                                            div()
                                                .pl(paddings.left)
                                                .pr(paddings.right)
                                                .child(header),
                                        )
                                    })
                                    // 标题区域
                                    .when_some(self.title, |this, title| {
                                        this.child(
                                            DialogTitle::new()
                                                .pl(paddings.left)
                                                .pr(paddings.right)
                                                .child(title),
                                        )
                                    })
                                    // 内容区域（通过构建器创建）
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
                                    // 子元素区域（可滚动）
                                    .when(!self.children.is_empty(), |this| {
                                        this.child(
                                            div().flex_1().overflow_hidden().child(
                                                // 主体内容
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
                            // 底部区域
                            .when_some(self.footer, |this, footer| {
                                this.child(div().pl(paddings.left).pr(paddings.right).child(footer))
                            })
                            // 关闭按钮（右上角）
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
                            // 阻止对话框内部点击事件冒泡
                            .on_any_mouse_down({
                                |_, _, cx| {
                                    cx.stop_propagation();
                                }
                            })
                            // 滑入动画
                            .with_animation("slide-down", animation.clone(), move |this, delta| {
                                // 等效于 `shadow_xl`，额外添加了透明度渐变
                                let shadow = vec![
                                    BoxShadow {
                                        color: hsla(0., 0., 0., 0.1 * delta),
                                        offset: point(px(0.), px(20.)),
                                        blur_radius: px(25.),
                                        spread_radius: px(-5.),
                                        inset: false,
                                    },
                                    BoxShadow {
                                        color: hsla(0., 0., 0., 0.1 * delta),
                                        offset: point(px(0.), px(8.)),
                                        blur_radius: px(10.),
                                        spread_radius: px(-6.),
                                        inset: false,
                                    },
                                ];
                                this.top(y * delta).shadow(shadow)
                            }),
                    )
                    // 淡入动画
                    .with_animation("fade-in", animation, move |this, delta| this.opacity(delta)),
            )
            .into_any_element()
    }
}
