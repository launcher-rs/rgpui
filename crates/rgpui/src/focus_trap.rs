//! 焦点陷阱 - 将 Tab 键循环限制在模态容器内。

use crate::{
    AnyElement, App, Bounds, Element, ElementId, FocusHandle, Global, GlobalElementId,
    InteractiveElement, Interactivity, IntoElement, LayoutId, ParentElement, Pixels,
    StatefulInteractiveElement, StyleRefinement, Styled, WeakFocusHandle, Window,
};
#[cfg(feature = "dom-backend")]
use crate::DomNode;
use std::collections::HashMap;

/// 为交互元素添加 `focus_trap` 能力的扩展 trait。
pub trait FocusTrapElement: InteractiveElement + Sized {
    /// 为元素启用焦点陷阱。
    ///
    /// 启用后，焦点会自动在该容器内循环而不会逃逸到父元素。适用于模态对话框、
    /// 侧边抽屉等覆盖层组件。
    ///
    /// 焦点陷阱工作方式：
    /// 1. 将该元素注册为焦点陷阱容器。
    /// 2. 按下 Tab/Shift-Tab 时 Root 拦截事件。
    /// 3. 如果焦点将离开容器，则循环回到容器的起点/终点。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// v_flex()
    ///     .child(Button::new("btn1").label("Button 1"))
    ///     .child(Button::new("btn2").label("Button 2"))
    ///     .child(Button::new("btn3").label("Button 3"))
    ///     .focus_trap("trap1", &self.container_focus_handle)
    /// // 按 Tab 循环：btn1 -> btn2 -> btn3 -> btn1
    /// // 焦点不会逃逸到容器外的元素
    /// ```
    fn focus_trap(
        self,
        id: impl Into<ElementId>,
        focus_handle: &FocusHandle,
    ) -> FocusTrapContainer<Self>
    where
        Self: ParentElement + Styled + Element + 'static,
    {
        FocusTrapContainer::new(id, focus_handle.clone(), self)
    }
}
impl<T: InteractiveElement + Sized> FocusTrapElement for T {}

/// 管理所有焦点陷阱容器的全局状态。
pub(crate) struct FocusTrapManager {
    /// 从容器元素 ID 到其焦点陷阱信息的映射。
    traps: HashMap<GlobalElementId, WeakFocusHandle>,
}

impl Global for FocusTrapManager {}

impl FocusTrapManager {
    /// 创建新的焦点陷阱管理器。
    fn new() -> Self {
        Self {
            traps: HashMap::new(),
        }
    }

    pub(crate) fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<FocusTrapManager>()
    }

    /// 注册一个焦点陷阱容器。
    fn register_trap(id: &GlobalElementId, container_handle: WeakFocusHandle, cx: &mut App) {
        // 惰性初始化：全局状态不存在时先创建，避免依赖外部 init 调用。
        if cx.try_global::<FocusTrapManager>().is_none() {
            cx.set_global(FocusTrapManager::new());
        }
        let this = Self::global_mut(cx);
        this.traps.insert(id.clone(), container_handle);
        this.cleanup();
    }

    /// 查找包含当前聚焦元素的焦点陷阱。
    pub(crate) fn find_active_trap(window: &Window, cx: &App) -> Option<FocusHandle> {
        let Some(manager) = cx.try_global::<FocusTrapManager>() else {
            return None;
        };
        for (_id, container_handle) in manager.traps.iter() {
            let Some(container) = container_handle.upgrade() else {
                continue;
            };

            if container.contains_focused(window, cx) {
                return Some(container);
            }
        }
        None
    }

    /// 清理句柄已释放的陷阱。
    fn cleanup(&mut self) {
        self.traps.retain(|_, handle| handle.upgrade().is_some());
    }
}

impl Default for FocusTrapManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 实现焦点陷阱行为的包装元素。
///
/// 包装另一个元素并将其注册为焦点陷阱容器。
/// 按下 Tab/Shift-Tab 时焦点会在容器内自动循环。
pub struct FocusTrapContainer<E: InteractiveElement + ParentElement + Styled + Element> {
    id: ElementId,
    focus_handle: FocusHandle,
    base: E,
}

impl<E: InteractiveElement + ParentElement + Styled + Element> FocusTrapContainer<E> {
    pub(crate) fn new(id: impl Into<ElementId>, focus_handle: FocusHandle, child: E) -> Self {
        Self {
            id: id.into(),
            base: child.track_focus(&focus_handle),
            focus_handle,
        }
    }
}

impl<E: InteractiveElement + ParentElement + Styled + Element> IntoElement
    for FocusTrapContainer<E>
{
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
impl<E: InteractiveElement + ParentElement + Styled + Element> ParentElement
    for FocusTrapContainer<E>
{
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}
impl<E: InteractiveElement + ParentElement + Styled + Element> InteractiveElement
    for FocusTrapContainer<E>
{
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}
impl<E: InteractiveElement + ParentElement + Styled + Element> StatefulInteractiveElement
    for FocusTrapContainer<E>
{
}
impl<E: InteractiveElement + ParentElement + Styled + Element> Styled for FocusTrapContainer<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl<E: InteractiveElement + ParentElement + Styled + Element + 'static> Element
    for FocusTrapContainer<E>
{
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    #[cfg(feature = "dom-backend")]
    fn dom(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) -> Option<DomNode> {
        self.base.dom(bounds, window, cx)
    }

    fn request_layout(
        &mut self,
        global_id: Option<&crate::GlobalElementId>,
        _inspector_id: Option<&crate::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        // 向管理器注册此焦点陷阱。
        FocusTrapManager::register_trap(global_id.unwrap(), self.focus_handle.downgrade(), cx);

        self.base.request_layout(global_id, None, window, cx)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&crate::GlobalElementId>,
        inspector_id: Option<&crate::InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.base
            .prepaint(global_id, inspector_id, bounds, request_layout, window, cx)
    }

    fn paint(
        &mut self,
        global_id: Option<&crate::GlobalElementId>,
        inspector_id: Option<&crate::InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.base.paint(
            global_id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        )
    }
}
