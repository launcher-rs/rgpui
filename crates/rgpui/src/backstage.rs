//! 应用菜单（Backstage）—— 覆盖整窗的「文件」面板（feature `ribbon`）。
//!
//! 左侧是页面导航（新建 / 打开 / 打印 / 选项 / 关于…），右侧是当前页内容；
//! 页面既可以**承载自定义内容**（`content`），也可以只是**一条命令**
//! （`command`，点击后经 [`App::dispatch_command`](crate::App::dispatch_command)
//! 派发并关闭面板）。
//!
//! 可见性由 [`BackstageState`] 统一管理：`open`/`close`/`toggle`。
//! 示例见 `examples/showcase/src/bin/backstage.rs`。
//!
//! ```ignore
//! use rgpui::backstage::{Backstage, BackstageState, BackstageTab};
//! use rgpui::{Context, IconName};
//!
//! let state = cx.new(BackstageState::new);
//! Backstage::new(&state, vec![
//!     BackstageTab::new("new", "新建")
//!         .icon(IconName::File)
//!         .content(|_window, cx| rgpui::v_flex().child("模板列表").into_any_element()),
//!     BackstageTab::new("save", "保存").icon(IconName::Check).command("app::Save"),
//! ])
//! ```

use std::rc::Rc;

use crate::util::FluentBuilder as _;
use crate::{
    ActiveTheme as _, AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Icon, IconName,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement, Pixels, RenderOnce,
    SharedString, Sizable as _, StatefulInteractiveElement as _, Styled, Window, div, h_flex, px,
    v_flex,
};

/// 应用菜单页面。
pub struct BackstageTab {
    id: SharedString,
    label: SharedString,
    icon: Option<IconName>,
    command: Option<SharedString>,
    content: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>>,
}

impl BackstageTab {
    /// 创建页面（仅标题）。
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            command: None,
            content: None,
        }
    }

    /// 设置导航图标。
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 设为「命令」页面：点击后派发该命令并关闭面板（与内容互斥，后设置者生效）。
    pub fn command(mut self, command: impl Into<SharedString>) -> Self {
        self.command = Some(command.into());
        self.content = None;
        self
    }

    /// 设为「内容」页面：右侧渲染该闭包（与命令互斥，后设置者生效）。
    pub fn content(
        mut self,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.content = Some(Rc::new(content));
        self.command = None;
        self
    }
}

/// 应用菜单状态：可见性、当前页面与焦点。
pub struct BackstageState {
    open: bool,
    active: usize,
    focus_handle: FocusHandle,
}

impl BackstageState {
    /// 创建状态（默认关闭）。
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            open: false,
            active: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    /// 焦点句柄：打开面板后应把焦点交给它，`Esc` 才能关闭。
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 面板是否打开。
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 当前页面索引。
    pub fn active(&self) -> usize {
        self.active
    }

    /// 切换当前页面。
    pub fn set_active(&mut self, index: usize, cx: &mut Context<Self>) {
        self.active = index;
        cx.notify();
    }

    /// 打开面板。
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            self.open = true;
            cx.notify();
        }
    }

    /// 关闭面板。
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    /// 切换面板可见性。
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }
}

/// 应用菜单元素：覆盖整窗的左导航 + 右内容。
#[derive(IntoElement)]
pub struct Backstage {
    state: Entity<BackstageState>,
    tabs: Vec<BackstageTab>,
    sidebar_width: Pixels,
}

impl Backstage {
    /// 创建应用菜单。
    pub fn new(state: &Entity<BackstageState>, tabs: Vec<BackstageTab>) -> Self {
        Self {
            state: state.clone(),
            tabs,
            sidebar_width: px(200.0),
        }
    }

    /// 设置左侧导航宽度。
    pub fn sidebar_width(mut self, width: Pixels) -> Self {
        self.sidebar_width = width;
        self
    }
}

impl RenderOnce for Backstage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if !self.state.read(cx).is_open() {
            return div().into_any_element();
        }

        // 颜色先取出（Hsla 为 Copy），避免后续 `content(window, cx)` 的可变借用冲突。
        let theme = cx.theme();
        let (popover, primary, primary_foreground, foreground, muted, secondary) = (
            theme.tokens.popover,
            theme.primary,
            theme.primary_foreground,
            theme.foreground,
            theme.muted,
            theme.secondary,
        );
        let active = self.state.read(cx).active();
        let focus_handle = self.state.read(cx).focus_handle();

        // 左侧导航（顶部是「返回」）。
        let back = window.listener_for(&self.state, |state, _: &ClickEvent, _window, cx| {
            state.close(cx)
        });
        let nav: Vec<AnyElement> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let selected = index == active;
                let item_foreground = if selected {
                    primary_foreground
                } else {
                    foreground
                };
                h_flex()
                    .id(tab.id.clone())
                    .items_center()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .text_color(item_foreground)
                    .when(selected, |this| this.bg(primary))
                    .when(!selected, |this| this.hover(|style| style.bg(secondary)))
                    .when_some(tab.icon.clone(), |this, icon| {
                        this.child(Icon::new(icon).with_size(px(16.0)))
                    })
                    .child(tab.label.clone())
                    .on_click(window.listener_for(
                        &self.state,
                        move |state, _: &ClickEvent, _, cx| {
                            state.set_active(index, cx);
                        },
                    ))
                    .into_any_element()
            })
            .collect();

        // 右侧内容：命令页点击后派发并关闭。
        let pane = match self.tabs.get(active) {
            Some(tab) => match (&tab.command, &tab.content) {
                (Some(command), _) => {
                    let command = command.clone();
                    let label = crate::CommandRegistry::global().label_for(&command);
                    div()
                        .id("backstage-command")
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded_md()
                        .cursor_pointer()
                        .text_sm()
                        .bg(primary)
                        .text_color(primary_foreground)
                        .child(format!("执行命令：{label}"))
                        .on_click(window.listener_for(
                            &self.state,
                            move |state, _: &ClickEvent, _window, cx| {
                                if let Err(error) = cx.dispatch_command(&command, None) {
                                    log::warn!("backstage：派发命令 `{command}` 失败：{error}");
                                }
                                state.close(cx);
                            },
                        ))
                        .into_any_element()
                }
                (None, Some(content)) => content(window, cx),
                (None, None) => div().text_sm().child(tab.label.clone()).into_any_element(),
            },
            None => div().into_any_element(),
        };

        div()
            .id("backstage")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_row()
            .bg(popover)
            .track_focus(&focus_handle)
            .key_context("Backstage")
            .on_key_down(window.listener_for(
                &self.state,
                |state, event: &KeyDownEvent, _window, cx| {
                    if event.keystroke.key == "escape" {
                        state.close(cx);
                    }
                },
            ))
            .child(
                v_flex()
                    .w(self.sidebar_width)
                    .h_full()
                    .flex_shrink_0()
                    .gap(px(2.0))
                    .p(px(8.0))
                    .bg(muted)
                    .child(
                        div()
                            .id("backstage-back")
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .text_color(foreground)
                            .hover(|style| style.bg(secondary))
                            .child("返回")
                            .on_click(back),
                    )
                    .children(nav),
            )
            .child(v_flex().flex_1().h_full().p(px(24.0)).child(pane))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{Backstage, BackstageState, BackstageTab};
    use crate::{
        AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
        Window, div,
    };

    /// 供渲染测试用的最小视图外壳。
    struct Probe {
        state: Entity<BackstageState>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().relative().size_full().child(Backstage::new(
                &self.state,
                vec![
                    BackstageTab::new("new", "新建")
                        .content(|_window, _cx| div().child("模板列表").into_any_element()),
                    BackstageTab::new("save", "保存").command("rgpui_test::Open"),
                ],
            ))
        }
    }

    /// 状态默认关闭，open/close/toggle 与页面切换生效。
    #[rgpui::test]
    fn state_open_close_toggle(cx: &mut crate::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| BackstageState::new(cx));
            Probe { state }
        });
        let state = view.read_with(cx, |probe, _| probe.state.clone());
        assert!(!state.read_with(cx, |state, _| state.is_open()));
        state.update(cx, |state, cx| state.open(cx));
        assert!(state.read_with(cx, |state, _| state.is_open()));
        state.update(cx, |state, cx| state.toggle(cx));
        assert!(!state.read_with(cx, |state, _| state.is_open()));
        state.update(cx, |state, cx| {
            state.open(cx);
            state.set_active(1, cx);
        });
        assert_eq!(state.read_with(cx, |state, _| state.active()), 1);
        state.update(cx, |state, cx| state.close(cx));
        assert!(!state.read_with(cx, |state, _| state.is_open()));
    }

    /// 关闭态渲染为空、打开态可绘制（覆盖内容页与命令页）。
    #[rgpui::test]
    fn renders_when_open(cx: &mut crate::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| BackstageState::new(cx));
            Probe { state }
        });
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        let state = view.read_with(cx, |probe, _| probe.state.clone());
        state.update(cx, |state, cx| {
            state.open(cx);
            state.set_active(1, cx);
        });
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }
}
