use crate::dialog::{ANIMATION_DURATION, CancelDialog, ConfirmDialog, Dialog};
use crate::elements::TooltipOverlay;
use crate::{
    ActiveTheme, AnyView, App, AppContext, Context, Entity, FocusHandle, FocusTrapManager,
    InteractiveElement, IntoElement, KeyBinding, ParentElement as _, Render, StyleRefinement,
    Styled, StyledExt as _, WeakFocusHandle, Window, div,
};
use std::rc::Rc;

actions!(root, [Tab, TabPrev]);

const CONTEXT: &str = "Root";

/// 窗口根视图 - 管理对话框层的全局状态（active_dialogs、焦点恢复等）。
///
/// 对话框（Dialog）由用户视图通过 [`Root::render_dialog_layer`] 手动挂载渲染，
/// Root 本身不直接渲染对话框，只负责维护活动对话框的列表与焦点链。
pub struct Root {
    /// 样式配置
    style: StyleRefinement,
    /// 窗口内容根视图
    view: AnyView,
    /// 活动对话框列表（栈，后进先出）
    pub(crate) active_dialogs: Vec<ActiveDialog>,
    /// 对话框关闭动画后需要恢复的焦点句柄
    pending_focus_restore: Option<WeakFocusHandle>,
    /// 全局工具提示覆盖层实体
    pub(crate) tooltip_overlay: Entity<TooltipOverlay>,
}

/// 活动对话框 - 保存焦点句柄与构建闭包。
#[derive(Clone)]
pub(crate) struct ActiveDialog {
    /// 对话框自身的焦点句柄
    focus_handle: FocusHandle,
    /// 打开对话框前的焦点句柄（关闭后恢复）
    previous_focused_handle: Option<WeakFocusHandle>,
    /// 对话框构建闭包
    builder: Rc<dyn Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static>,
}

impl ActiveDialog {
    pub(crate) fn new(
        focus_handle: FocusHandle,
        previous_focused_handle: Option<WeakFocusHandle>,
        builder: impl Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
    ) -> Self {
        Self {
            focus_handle,
            previous_focused_handle,
            builder: Rc::new(builder),
        }
    }
}

impl Root {
    /// 创建新的 Root 视图。
    pub fn new(view: impl Into<AnyView>, cx: &mut Context<Self>) -> Self {
        // 直接将按键绑定写入 keymap，而不走 `bind_keys`。
        // `bind_keys` 会推送 `Effect::RefreshWindows`，在窗口创建阶段会对刚绘制的窗口再次触发
        // 重绘，导致多余的一帧渲染；Root 的按键绑定是静态的，无需刷新已存在的窗口。
        cx.key_bindings().borrow_mut().add_bindings([
            KeyBinding::new("tab", Tab, Some(CONTEXT)),
            KeyBinding::new("shift-tab", TabPrev, Some(CONTEXT)),
            KeyBinding::new("escape", CancelDialog, Some("Dialog")),
            KeyBinding::new("enter", ConfirmDialog, Some("Dialog")),
        ]);
        Self {
            style: StyleRefinement::default(),
            view: view.into(),
            active_dialogs: Vec::new(),
            pending_focus_restore: None,
            tooltip_overlay: cx.new(|_| TooltipOverlay::new()),
        }
    }

    /// 在窗口中更新 Root 视图。
    pub fn update<F, R>(window: &mut Window, cx: &mut App, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> R,
    {
        let root = window
            .root::<Root>()
            .flatten()
            .expect("BUG: window first layer should be a rgpui::Root.");

        root.update(cx, |root, cx| f(root, window, cx))
    }

    /// 只读访问窗口中的 Root 视图。
    pub fn read<'a>(window: &'a Window, cx: &'a App) -> &'a Self {
        &window
            .root::<Root>()
            .expect("The window root view should be of type `rgpui::Root`.")
            .unwrap()
            .read(cx)
    }

    /// 获取窗口的工具提示覆盖层实体。
    pub(crate) fn tooltip_overlay(window: &Window, cx: &App) -> Option<Entity<TooltipOverlay>> {
        let root = window.root::<Root>()??;
        Some(root.read(cx).tooltip_overlay.clone())
    }

    /// 返回 Root 的内容视图。
    pub fn view(&self) -> &AnyView {
        &self.view
    }

    /// 从窗口根视图中提取用户视图。
    ///
    /// 窗口根视图可能是包装过的 `Root`（自动或用户手动包装），也可能是用户视图本身。
    /// 此函数优先尝试直接 downcast 到目标类型 `V`，若失败则尝试穿透 `Root` 包装
    /// 取其内部视图再 downcast。返回 `Ok` 时给出用户视图实体，`Err` 返回原视图。
    pub(crate) fn root_view_downcast<V: 'static>(
        root_view: AnyView,
        cx: &App,
    ) -> Result<Entity<V>, AnyView> {
        if let Ok(view) = root_view.clone().downcast::<V>() {
            return Ok(view);
        }
        if let Ok(root) = root_view.clone().downcast::<Root>() {
            if let Ok(view) = root.read(cx).view().clone().downcast::<V>() {
                return Ok(view);
            }
        }
        Err(root_view)
    }

    /// 渲染对话框层。
    ///
    /// 由用户视图手动调用并挂载到渲染树中（Root 自身不挂载）。
    pub fn render_dialog_layer(
        window: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement + use<>> {
        let root = window.root::<Root>()??;

        let active_dialogs = root.read(cx).active_dialogs.clone();

        if active_dialogs.is_empty() {
            return None;
        }

        let mut show_overlay_ix = None;

        let mut dialogs = active_dialogs
            .iter()
            .enumerate()
            .map(|(i, active_dialog)| {
                let mut dialog = Dialog::new(cx);

                dialog = (active_dialog.builder)(dialog, window, cx);

                // 将焦点句柄交给对话框，因为 `dialog` 是临时值，无法保留焦点句柄，
                // 所以焦点句柄由 Root 持有的 `active_dialog` 提供。
                dialog.focus_handle = active_dialog.focus_handle.clone();

                dialog.layer_ix = i;
                // 找出需要显示遮罩的对话框。
                if dialog.has_overlay() {
                    show_overlay_ix = Some(i);
                }

                dialog
            })
            .collect::<Vec<_>>();

        if let Some(ix) = show_overlay_ix {
            if let Some(dialog) = dialogs.get_mut(ix) {
                dialog.props.overlay_visible = true;
            }
        }

        Some(div().children(dialogs))
    }

    /// 打开一个对话框。
    pub fn open_dialog<F>(&mut self, build: F, window: &mut Window, cx: &mut Context<'_, Root>)
    where
        F: Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
    {
        let mut previous_focused_handle = window.focused(cx).map(|h| h.downgrade());

        // 若存在待恢复的焦点句柄（关闭动画中），优先作为前一个焦点，维持焦点链。
        if let Some(pending_handle) = self.pending_focus_restore.take() {
            previous_focused_handle = Some(pending_handle);
        }

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        self.active_dialogs.push(ActiveDialog::new(
            focus_handle,
            previous_focused_handle,
            build,
        ));
        cx.notify();
    }

    /// 关闭对话框（立即恢复焦点）。
    pub fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<'_, Root>) {
        if let Some(handle) = self.close_dialog_internal() {
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    /// 延迟关闭对话框（等待动画结束后恢复焦点）。
    pub(crate) fn defer_close_dialog(&mut self, window: &mut Window, cx: &mut Context<'_, Root>) {
        if let Some(handle) = self.close_dialog_internal() {
            let dialogs_count = self.active_dialogs.len();

            // 动画期间新打开的对话框保持焦点链。
            self.pending_focus_restore = Some(handle.downgrade());

            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(*ANIMATION_DURATION).await;
                let _ = this.update_in(cx, |this, window, cx| {
                    let current_dialogs_count = this.active_dialogs.len();
                    // 仅当动画期间没有打开新对话框时才恢复焦点。
                    if current_dialogs_count == dialogs_count {
                        window.focus(&handle, cx);
                    }
                    this.pending_focus_restore = None;
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// 关闭所有对话框。
    pub fn close_all_dialogs(&mut self, window: &mut Window, cx: &mut Context<'_, Root>) {
        let previous_focused_handle = self
            .active_dialogs
            .first()
            .and_then(|d| d.previous_focused_handle.clone());
        self.active_dialogs.clear();
        if let Some(handle) = previous_focused_handle.and_then(|h| h.upgrade()) {
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    fn close_dialog_internal(&mut self) -> Option<FocusHandle> {
        self.active_dialogs
            .pop()
            .and_then(|d| d.previous_focused_handle)
            .and_then(|h| h.upgrade())
    }

    /// 处理 Tab 键：若在焦点陷阱内则循环焦点，否则正常切换。
    fn on_action_tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        // 检查是否位于焦点陷阱内。
        if let Some(container_focus_handle) = FocusTrapManager::find_active_trap(window, cx) {
            // 记录切换前焦点，尝试正常导航后检测是否逃逸陷阱。
            let before_focus = window.focused(cx);

            window.focus_next(cx);

            if !container_focus_handle.contains_focused(window, cx) {
                // 逃逸陷阱后循环回陷阱开头。
                let mut attempts = 0;
                const MAX_ATTEMPTS: usize = 100;

                while !container_focus_handle.contains_focused(window, cx)
                    && attempts < MAX_ATTEMPTS
                {
                    window.focus_next(cx);
                    attempts += 1;

                    if window.focused(cx) == before_focus {
                        break;
                    }
                }
            }
            return;
        }

        window.focus_next(cx);
    }

    /// 处理 Shift+Tab 键：焦点陷阱内反向循环。
    fn on_action_tab_prev(&mut self, _: &TabPrev, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(container_focus_handle) = FocusTrapManager::find_active_trap(window, cx) {
            let before_focus = window.focused(cx);

            window.focus_prev(cx);

            if !container_focus_handle.contains_focused(window, cx) {
                let mut attempts = 0;
                const MAX_ATTEMPTS: usize = 100;

                while !container_focus_handle.contains_focused(window, cx)
                    && attempts < MAX_ATTEMPTS
                {
                    window.focus_prev(cx);
                    attempts += 1;

                    if window.focused(cx) == before_focus {
                        break;
                    }
                }
            }
            return;
        }

        window.focus_prev(cx);
    }
}

impl Styled for Root {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(cx.theme().font_size);

        div()
            .id("root")
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::on_action_tab))
            .on_action(cx.listener(Self::on_action_tab_prev))
            .relative()
            .size_full()
            .grid()
            // 用 1fr 网格轨道容纳子视图，使 auto 尺寸的子视图拉伸填满窗口
            // （与窗口根元素的拉伸语义一致），显式尺寸的子视图则保持自身尺寸。
            .grid_cols(1)
            .grid_rows(1)
            .font_family(cx.theme().font_family.clone())
            .bg(cx.theme().tokens.background)
            .text_color(cx.theme().foreground)
            .refine_style(&self.style)
            .child(self.view.clone())
            .child(self.tooltip_overlay.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    /// 测试 Root 创建
    #[test]
    fn test_root_creation() {
        // 在无窗口上下文中验证类型可用性
        let _ = std::any::type_name::<Root>;
        let _ = std::any::type_name::<TestView>;
    }
}
