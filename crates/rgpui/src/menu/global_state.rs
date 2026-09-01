//! 菜单全局状态，存储应用菜单和延迟渲染弹层的全局信息。

use crate::{App, ElementId, FocusHandle, Global, OwnedMenu};
use std::cell::Cell;
use std::collections::HashSet;

/// 全局状态 - 存储应用菜单与延迟渲染的弹层信息
pub struct GlobalState {
    /// 使用延迟渲染的弹层 ID 集合。
    ///
    /// 当此集合非空时，表示当前处于至少一个延迟渲染上下文中。
    /// 用于防止重复延迟元素导致 rgpui 发生 panic。
    open_deferred_popovers: HashSet<ElementId>,
    /// 应用程序菜单存储
    app_menus: Vec<OwnedMenu>,
    /// 是否抑制窗口级文本选择。
    ///
    /// 输入框拥有自己的文本选择逻辑，当鼠标在输入框内按下时设置此标志，
    /// 防止窗口级文本选择（Root）启动拖拽选择。
    suppress_text_selection: Cell<bool>,
}

impl Global for GlobalState {}

impl GlobalState {
    /// 创建新的全局状态实例
    pub(crate) fn new() -> Self {
        Self {
            open_deferred_popovers: HashSet::new(),
            app_menus: Vec::new(),
            suppress_text_selection: Cell::new(false),
        }
    }

    /// 获取全局状态引用
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// 获取全局状态可变引用
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// 注册一个使用延迟渲染的弹层为打开状态
    pub(crate) fn register_deferred_popover(&mut self, focus_handle: &FocusHandle) {
        self.open_deferred_popovers
            .insert(format!("{focus_handle:?}").into());
    }

    /// 弹层关闭时注销
    pub(crate) fn unregister_deferred_popover(&mut self, focus_handle: &FocusHandle) {
        let element_id: ElementId = format!("{focus_handle:?}").into();
        self.open_deferred_popovers.remove(&element_id);
    }

    /// 获取应用程序菜单
    pub fn app_menus(&self) -> &[OwnedMenu] {
        &self.app_menus
    }

    /// 设置应用程序菜单
    pub fn set_app_menus(&mut self, menus: Vec<OwnedMenu>) {
        self.app_menus = menus;
    }

    /// 抑制窗口级文本选择，直到调用 [`Self::clear_suppress_text_selection`]。
    pub fn suppress_text_selection(cx: &mut App) {
        Self::global_mut(cx).suppress_text_selection.set(true);
    }

    /// 清除窗口级文本选择抑制标志。
    pub fn clear_suppress_text_selection(cx: &mut App) {
        Self::global_mut(cx).suppress_text_selection.set(false);
    }

    /// 是否抑制窗口级文本选择。
    pub fn is_suppress_text_selection(cx: &App) -> bool {
        Self::global(cx).suppress_text_selection.get()
    }
}
