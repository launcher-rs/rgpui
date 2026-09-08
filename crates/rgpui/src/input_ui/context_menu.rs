//! 输入框右键菜单：默认菜单 + 用户自定义扩展。
//!
//! [`Input`](super::Input) / [`InputState`](super::InputState) 默认带右键菜单
//! （剪切/复制/粘贴/全选/撤销/重做，按状态自动禁用），经现有
//! [`ContextMenu`](crate::menu::ContextMenu) 实现，无需应用层手动接线。
//!
//! 用户定制（三档，由简到繁）：
//!
//! ```ignore
//! use rgpui::input_ui::InputState;
//!
//! // 1. 开关（一行关掉，编辑器/搜索框等只读展示场景用）。
//! Input::new(&state).show_context_menu(false);
//!
//! // 2. 追加自定义项（默认项保留，自定义项跟在分隔符后，最常用）。
//! Input::new(&state).context_menu_extra(|menu, _state, _window, _cx| {
//!     menu.menu("转为大写", Box::new(Uppercase))
//! });
//!
//! // 3. 完全接管（默认项不要，自己从零构建；想复用默认项时调
//! //    `InputState::build_default_context_menu` 拼进去）。
//! Input::new(&state).context_menu_override(|menu, state, _window, cx| {
//!     let menu = InputState::build_default_context_menu(menu, &state, cx);
//!     menu.separator().menu("我的动作", Box::new(MyAction))
//! });
//! ```
//!
//! `Input` 层的设置会写入共享的 [`InputState`](super::InputState)（粘性，
//! 后设置的生效）；裸 `Entity<InputState>` 直渲染同样生效。

use std::rc::Rc;

use crate::menu::PopupMenu;
use crate::{App, Context, Entity, Window};

use super::InputState;
use super::{Copy, Cut, Paste, Redo, SelectAll, Undo};

/// 输入框右键菜单构建器：接默认菜单之后（或接管时从零）继续构建。
///
/// - `menu`：已含默认项的菜单（接管模式下为空菜单，由用户全权构建）。
/// - `state`：输入框状态实体，可读选中/撤销等状态做动态菜单。
pub type InputContextMenuBuilder =
    Rc<dyn Fn(PopupMenu, Entity<InputState>, &mut Window, &mut App) -> PopupMenu>;

impl InputState {
    /// 构建默认右键菜单（剪切/复制/粘贴/全选 + 分隔 + 撤销/重做）。
    ///
    /// 按当前状态自动禁用不可用项（无选区时剪切/复制禁用，空文本时全选禁用，
    /// 禁用输入框时剪切/粘贴/撤销/重做禁用）。供完全接管模式复用，
    /// 也供内部默认渲染使用。
    pub fn build_default_context_menu(
        menu: PopupMenu,
        state: &Entity<InputState>,
        cx: &App,
    ) -> PopupMenu {
        let (has_selection, is_empty, can_undo, can_redo, disabled) = {
            let snapshot = state.read(cx);
            (
                snapshot.has_selection(),
                snapshot.is_empty(),
                snapshot.can_undo(),
                snapshot.can_redo(),
                // 只读与禁用一样不可写（复制/全选仍可用）。
                snapshot.is_disabled() || snapshot.is_read_only(),
            )
        };
        menu.menu_with_disabled("剪切", Box::new(Cut), !has_selection || disabled)
            .menu_with_disabled("复制", Box::new(Copy), !has_selection)
            .menu_with_disabled("粘贴", Box::new(Paste), disabled)
            .menu_with_disabled("全选", Box::new(SelectAll), is_empty)
            .separator()
            .menu_with_disabled("撤销", Box::new(Undo), !can_undo || disabled)
            .menu_with_disabled("重做", Box::new(Redo), !can_redo || disabled)
    }

    /// 内部使用：按当前配置构建完整菜单（接管优先，否则默认 + 追加项）。
    pub(super) fn build_context_menu(
        menu: PopupMenu,
        state: &Entity<InputState>,
        window: &mut Window,
        cx: &mut App,
    ) -> PopupMenu {
        let (override_builder, extra_builder) = {
            let snapshot = state.read(cx);
            (
                snapshot.context_menu_override.clone(),
                snapshot.context_menu_extra.clone(),
            )
        };
        let menu = match override_builder {
            Some(build) => build(menu, state.clone(), window, cx),
            None => Self::build_default_context_menu(menu, state, cx),
        };
        match extra_builder {
            // 追加项前统一加分隔符（菜单为空或末尾已是分隔符时 `separator` 是空操作）。
            Some(build) => build(menu.separator(), state.clone(), window, cx),
            None => menu,
        }
    }

    /// 是否有非空选区（右键菜单剪切/复制的启用依据）。
    pub fn has_selection(&self) -> bool {
        !self.core.selected_range.is_empty()
    }

    /// 文本是否为空（右键菜单全选的启用依据）。
    pub fn is_empty(&self) -> bool {
        self.core.text.len() == 0
    }

    /// 输入框是否被禁用（右键菜单剪切/粘贴/撤销/重做的禁用依据）。
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// 是否可撤销（右键菜单撤销项的启用依据）。
    pub fn can_undo(&self) -> bool {
        !self.core.history.undos().is_empty()
    }

    /// 是否可重做（右键菜单重做项的启用依据）。
    pub fn can_redo(&self) -> bool {
        !self.core.history.redos().is_empty()
    }

    /// 右键菜单总开关（builder 版，创建时链式调用）。
    pub fn show_context_menu(mut self, show: bool) -> Self {
        self.context_menu_enabled = show;
        self
    }

    /// 右键菜单总开关（创建后修改）。
    pub fn set_context_menu_enabled(&mut self, show: bool, cx: &mut Context<Self>) {
        self.context_menu_enabled = show;
        cx.notify();
    }

    /// 右键菜单是否启用。
    pub fn is_context_menu_enabled(&self) -> bool {
        self.context_menu_enabled
    }

    /// 在默认菜单后追加自定义项（builder 版，创建时链式调用，最常用）。
    pub fn context_menu_extra<F>(mut self, builder: F) -> Self
    where
        F: Fn(PopupMenu, Entity<InputState>, &mut Window, &mut App) -> PopupMenu + 'static,
    {
        self.context_menu_extra = Some(Rc::new(builder));
        self
    }

    /// 在默认菜单后追加自定义项（创建后修改）。
    pub fn set_context_menu_extra<F>(&mut self, builder: F, cx: &mut Context<Self>)
    where
        F: Fn(PopupMenu, Entity<InputState>, &mut Window, &mut App) -> PopupMenu + 'static,
    {
        self.context_menu_extra = Some(Rc::new(builder));
        cx.notify();
    }

    /// 完全接管右键菜单（builder 版，不再显示默认项，除非手动调
    /// [`Self::build_default_context_menu`] 拼回去）。
    pub fn context_menu_override<F>(mut self, builder: F) -> Self
    where
        F: Fn(PopupMenu, Entity<InputState>, &mut Window, &mut App) -> PopupMenu + 'static,
    {
        self.context_menu_override = Some(Rc::new(builder));
        self
    }

    /// 完全接管右键菜单（创建后修改）。
    pub fn set_context_menu_override<F>(&mut self, builder: F, cx: &mut Context<Self>)
    where
        F: Fn(PopupMenu, Entity<InputState>, &mut Window, &mut App) -> PopupMenu + 'static,
    {
        self.context_menu_override = Some(Rc::new(builder));
        cx.notify();
    }

    /// 清除追加/接管两档自定义，恢复纯默认菜单。
    pub fn clear_context_menu_customization(&mut self, cx: &mut Context<Self>) {
        self.context_menu_extra = None;
        self.context_menu_override = None;
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use std::cell::Cell;

    /// 持有输入框状态的测试宿主视图。
    struct Probe {
        state: Entity<InputState>,
    }

    impl crate::Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            crate::div()
        }
    }

    #[rgpui::test]
    fn default_menu_tracks_state(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| Probe {
            state: cx.new(|cx| InputState::new(window, cx).default_value("hello")),
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());

        // 初始：非空、无选区、无撤销。
        assert!(!state.read_with(cx, |state, _| state.is_empty()));
        assert!(!state.read_with(cx, |state, _| state.has_selection()));
        assert!(!state.read_with(cx, |state, _| state.can_undo()));
        // 默认菜单恒非空（禁用项仍展示，只是点不了）。
        cx.update(|_, cx| {
            let menu = InputState::build_default_context_menu(PopupMenu::new(cx), &state, cx);
            assert!(!menu.is_empty());
        });

        // 全选后有选区。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..5, cx));
        });
        assert!(state.read_with(cx, |state, _| state.has_selection()));

        // 编辑后可撤销。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.replace("hi", window, cx));
        });
        assert!(state.read_with(cx, |state, _| state.can_undo()));
    }

    #[rgpui::test]
    fn custom_builders_are_invoked(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| Probe {
            state: cx.new(|cx| InputState::new(window, cx).default_value("hello")),
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());

        let extra_called = Rc::new(Cell::new(false));
        let override_called = Rc::new(Cell::new(false));
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_context_menu_extra(
                    {
                        let extra_called = extra_called.clone();
                        move |menu, _, _, _| {
                            extra_called.set(true);
                            menu
                        }
                    },
                    cx,
                );
                state.set_context_menu_override(
                    {
                        let override_called = override_called.clone();
                        move |menu, _, _, _| {
                            override_called.set(true);
                            menu.label("自定义")
                        }
                    },
                    cx,
                );
            });
        });

        // 接管 + 追加同时设置时两者都触发（追加跟在接管结果之后）。
        cx.update(|window, cx| {
            let menu = InputState::build_context_menu(PopupMenu::new(cx), &state, window, cx);
            assert!(!menu.is_empty());
        });
        assert!(extra_called.get());
        assert!(override_called.get());

        // 清除后恢复纯默认菜单。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.clear_context_menu_customization(cx));
        });
        cx.update(|window, cx| {
            let menu = InputState::build_context_menu(PopupMenu::new(cx), &state, window, cx);
            assert!(!menu.is_empty());
        });
    }

    /// `Input` 元素层的三档设置会写入共享 state 并随渲染生效。
    struct InputProbe {
        state: Entity<InputState>,
    }

    impl crate::Render for InputProbe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            super::super::Input::new(&self.state)
                .show_context_menu(false)
                .context_menu_extra(|menu, _, _, _| menu)
        }
    }

    #[rgpui::test]
    fn input_element_writes_menu_config_to_state(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| InputProbe {
            state: cx.new(|cx| InputState::new(window, cx).default_value("hello")),
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        // 元素层渲染后：开关写入 false，追加项写入 Some。
        let (enabled, has_extra) = state.read_with(cx, |state, _| {
            (
                state.is_context_menu_enabled(),
                state.context_menu_extra.is_some(),
            )
        });
        assert!(!enabled);
        assert!(has_extra);
    }
}
