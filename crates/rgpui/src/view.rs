//! 视图系统 —— 提供 AnyView、ViewContext 等核心视图抽象与渲染管线。

use crate::collections::FxHashSet;
use crate::refineable::Refineable;
use crate::{
    AnyElement, AnyEntity, AnyWeakEntity, App, Bounds, ContentMask, Context, Element, ElementId,
    Entity, EntityId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, PaintIndex,
    Pixels, PrepaintStateIndex, Render, RenderOnce, Style, StyleRefinement, TextStyle, WeakEntity,
};
use crate::{Empty, Window};
use anyhow::Result;
use std::mem;
use std::{any::TypeId, fmt, ops::Range};

/// 可向下转型为特定 `Entity<V>` 的动态类型视图句柄。
///
/// 这是 [`ViewElement`] 的类型擦除对应物：它持有一个实体及其渲染函数指针，
/// 本身也是一个 [`View`]，因此将其作为元素嵌入时会经过与任何其他视图相同的
/// [`ViewElement`] 机制。
#[derive(Clone, Debug)]
pub struct AnyView {
    entity: AnyEntity,
    render: fn(&AnyView, &mut Window, &mut App) -> AnyElement,
}

impl<V: Render> From<Entity<V>> for AnyView {
    fn from(value: Entity<V>) -> Self {
        AnyView {
            entity: value.into_any(),
            render: any_view::render::<V>,
        }
    }
}

impl AnyView {
    /// 将此视图嵌入为使用 `style` 布局的缓存 [`ViewElement`]。
    ///
    /// 渲染的子树将从上一帧回收，除非自渲染以来在支撑实体上调用了
    /// [Context::notify]（或调用了 [Window::refresh]，它会忽略缓存）。
    pub fn cached(self, style: StyleRefinement) -> ViewElement<AnyView> {
        ViewElement::new(self).cached(style)
    }

    /// 转换为弱句柄。
    pub fn downgrade(&self) -> AnyWeakView {
        AnyWeakView {
            entity: self.entity.downgrade(),
            render: self.render,
        }
    }

    /// 转换为特定类型的 [Entity]。
    /// 如果此句柄不包含指定类型的视图，则在 `Err` 变体中返回自身。
    pub fn downcast<T: 'static>(self) -> Result<Entity<T>, Self> {
        match self.entity.downcast() {
            Ok(entity) => Ok(entity),
            Err(entity) => Err(Self {
                entity,
                render: self.render,
            }),
        }
    }

    /// 获取底层视图的 [TypeId]。
    pub fn entity_type(&self) -> TypeId {
        self.entity.entity_type
    }

    /// 此视图的 [`EntityId`]。
    pub fn entity_id(&self) -> EntityId {
        self.entity.entity_id()
    }
}

impl PartialEq for AnyView {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}

impl Eq for AnyView {}

/// `AnyView` 是 [`View`] 的类型擦除版本：其 `render` 是函数指针而非具体类型，
/// 但它通过 [`ViewElement`] 与任何其他视图一样参与响应式图。
impl View for AnyView {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.entity.entity_id())
    }

    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        (self.render)(&self, window, cx)
    }
}

impl<V: 'static + Render> IntoElement for Entity<V> {
    type Element = ViewElement<Entity<V>>;

    fn into_element(self) -> Self::Element {
        ViewElement::new(self)
    }
}

impl IntoElement for AnyView {
    type Element = ViewElement<AnyView>;

    fn into_element(self) -> Self::Element {
        ViewElement::new(self)
    }
}

/// 弱引用的动态类型视图句柄。
pub struct AnyWeakView {
    entity: AnyWeakEntity,
    render: fn(&AnyView, &mut Window, &mut App) -> AnyElement,
}

impl AnyWeakView {
    /// 如果视图仍然存活，升级为强引用 `AnyView` 句柄。
    pub fn upgrade(&self) -> Option<AnyView> {
        let entity = self.entity.upgrade()?;
        Some(AnyView {
            entity,
            render: self.render,
        })
    }
}

impl<V: 'static + Render> From<WeakEntity<V>> for AnyWeakView {
    fn from(view: WeakEntity<V>) -> Self {
        AnyWeakView {
            entity: view.into(),
            render: any_view::render::<V>,
        }
    }
}

impl PartialEq for AnyWeakView {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}

impl std::fmt::Debug for AnyWeakView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyWeakView")
            .field("entity_id", &self.entity.entity_id)
            .finish_non_exhaustive()
    }
}

mod any_view {
    use crate::{AnyElement, AnyView, App, IntoElement, Render, Window};

    pub(crate) fn render<V: 'static + Render>(
        view: &AnyView,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let view = view.clone().downcast::<V>().unwrap();
        // Record the view's Render type name so the accessibility debug dump can
        // attribute nodes to the view that produced them.
        #[cfg(debug_assertions)]
        window
            .a11y
            .view_type_names
            .insert(view.entity_id(), std::any::type_name::<V>());
        view.update(cx, |view, cx| view.render(window, cx).into_any_element())
    }
}

/// 参与 RGPUI 响应式图的可渲染物 — 这是 [`Render`] 和 [`RenderOnce`] 背后的统一模型。
///
/// 当 `entity_id()` 返回 `Some` 时，该 id 成为视图的标识：它获得一个唯一的
/// 元素 id 空间（因此内部的 `use_state` / `.id(..)` 不会在兄弟节点间冲突），
/// 并且在该实体上执行 `cx.notify()` 只会重新渲染此视图的子树。
/// `None` 的行为类似无状态组件。
///
/// 很少直接实现 `View`。`Entity<T: Render>` 和任何 `T: RenderOnce`
/// 在下面有泛型实现；仅当组件同时需要父级提供的属性*和*用于标识的支撑实体时
/// 才需要手动实现。
pub trait View: 'static + Sized {
    /// 此视图的标识（如果有的话）。视图通常将支撑实体作为字段持有，
    /// 并在此处返回其 [`EntityId`]。
    ///
    /// 该 id 成为此视图的 [`ElementId`]，因此以同一实体为键的两个视图
    /// 不得在元素树中的相同位置渲染（例如作为同一父节点下的子节点）：
    /// 它们的内部元素状态（`use_state`、滚动偏移等）会静默冲突。
    /// 嵌套是可以的 — id 由父路径限定作用域。
    fn entity_id(&self) -> Option<EntityId>;

    /// 将此视图渲染为元素树，消耗 `self`。
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement;
}

/// 无状态组件（`RenderOnce`）是没有标识的 `View`。
impl<T: RenderOnce> View for T {
    fn entity_id(&self) -> Option<EntityId> {
        None
    }

    #[inline]
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        RenderOnce::render(self, window, cx)
    }
}

/// 渲染自身的实体（`Render`）是以自身 id 为键的 `View`。
impl<T: Render> View for Entity<T> {
    fn entity_id(&self) -> Option<EntityId> {
        Some(Entity::entity_id(self))
    }

    #[inline]
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.update(cx, |this, cx| {
            Render::render(this, window, cx).into_any_element()
        })
    }
}

impl<T: Render> Entity<T> {
    /// 将此实体嵌入为使用 `style` 布局的缓存 [`ViewElement`]。
    ///
    /// 渲染的子树将被复用，直到实体收到通知（或缓存的边界/文本样式更改）。
    /// 缓存需要确定的大小：缓存的视图从 `style` 布局，而*不是*从其内容测量。
    /// 对于非缓存情况，请使用 [`ViewElement::new`]（或 `.child(entity)`）。
    #[track_caller]
    pub fn cached(self, style: StyleRefinement) -> ViewElement<Entity<T>> {
        ViewElement::new(self).cached(style)
    }
}

/// [`View`] 实现的元素类型。包装一个 `View` 并将其挂接到布局、预绘制和绘制中。
/// 通过 [`ViewElement::new`] 构造。
#[doc(hidden)]
pub struct ViewElement<V: View> {
    view: Option<V>,
    entity_id: Option<EntityId>,
    cached_style: Option<StyleRefinement>,
    /// 布局阶段记录的访问集（Retained P2a）：供 prepaint 存入元素状态，复用时重放。
    pending_accessed_entities: Option<FxHashSet<EntityId>>,
    /// 布局阶段记录的依赖快照（Retained P2a）：与 prepaint／paint 期记录合并后存储。
    pending_dependencies: Option<crate::fast::dependencies::DependencySet>,
    #[cfg(debug_assertions)]
    source: &'static core::panic::Location<'static>,
}

impl<V: View> ViewElement<V> {
    /// 将 [`View`] 包装为元素。
    #[track_caller]
    pub fn new(view: V) -> Self {
        let entity_id = view.entity_id();
        ViewElement {
            entity_id,
            cached_style: None,
            view: Some(view),
            pending_accessed_entities: None,
            pending_dependencies: None,
            #[cfg(debug_assertions)]
            source: core::panic::Location::caller(),
        }
    }

    /// 启用此视图渲染子树的缓存，使用 `style` 布局。
    /// 组合器提供布局样式，因为缓存会跳过渲染内容来测量它们。
    ///
    /// 有意设为 crate 私有：缓存仅对实体支撑的视图是健全的，
    /// 其中 [`Context::notify`] 是打破缓存的契约。无状态视图没有这样的契约，
    /// 因此冻结的子树永远无法失效。
    /// 通过 [`Entity::cached`] 或 [`AnyView::cached`] 到达此处，
    /// 它们在构造时就是实体支撑的。
    pub(crate) fn cached(mut self, style: StyleRefinement) -> Self {
        self.cached_style = Some(style);
        self
    }
}

impl<V: View> IntoElement for ViewElement<V> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct ViewElementState {
    prepaint_range: Range<PrepaintStateIndex>,
    paint_range: Range<PaintIndex>,
    cache_key: ViewElementCacheKey,
    accessed_entities: FxHashSet<EntityId>,
    /// 上次绘制记录的依赖快照（Retained P1）：读过的实体／全局变化即旁路复用。
    /// 只会多重建、不会复用过期帧；paint 阶段读取 P2 再纳入。
    dependencies: crate::fast::dependencies::DependencySet,
}

struct ViewElementCacheKey {
    bounds: Bounds<Pixels>,
    content_mask: ContentMask<Pixels>,
    text_style: TextStyle,
}

impl<V: View> Element for ViewElement<V> {
    type RequestLayoutState = Option<AnyElement>;
    type PrepaintState = Option<AnyElement>;

    fn id(&self) -> Option<ElementId> {
        self.entity_id.map(ElementId::View)
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        #[cfg(debug_assertions)]
        return Some(self.source);

        #[cfg(not(debug_assertions))]
        return None;
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        if let Some(entity_id) = self.entity_id {
            // Stateful path: create a reactive boundary.
            window.with_rendered_view(entity_id, |window| {
                // 检查器打开期间禁用视图缓存，保证树/hitbox 完整新鲜；
                // 关闭时零额外开销（条件恒为 false）。
                #[cfg(any(feature = "inspector", debug_assertions))]
                let caching_disabled = window.is_inspector_open();
                #[cfg(not(any(feature = "inspector", debug_assertions)))]
                let caching_disabled = false;
                match self.cached_style.as_ref() {
                    Some(style) if !caching_disabled => {
                        let mut root_style = Style::default();
                        root_style.refine(style);
                        let layout_id = window.request_layout(root_style, None, cx);
                        (layout_id, None)
                    }
                    _ => {
                        // Retained P2a：渲染期同步记录访问集与依赖快照，
                        // 供 prepaint 复用判定与元素状态存储。
                        cx.begin_dependency_recording();
                        let ((element, layout_id), accessed_entities) = cx
                            .detect_accessed_entities(|cx| {
                                let mut element = self
                                    .view
                                    .take()
                                    .unwrap()
                                    .render(window, cx)
                                    .into_any_element();
                                let layout_id = element.request_layout(window, cx);
                                (element, layout_id)
                            });
                        let dependencies = cx.end_dependency_recording();
                        self.pending_accessed_entities = Some(accessed_entities);
                        self.pending_dependencies = Some(dependencies);
                        (layout_id, Some(element))
                    }
                }
            })
        } else {
            // Stateless path: isolate subtree via type name (no entity identity).
            window.with_id(
                ElementId::Name(std::any::type_name::<V>().into()),
                |window| {
                    let mut element = self
                        .view
                        .take()
                        .unwrap()
                        .render(window, cx)
                        .into_any_element();
                    let layout_id = element.request_layout(window, cx);
                    (layout_id, Some(element))
                },
            )
        }
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        if let Some(entity_id) = self.entity_id {
            // Stateful path.
            window.set_view_id(entity_id);
            window.with_rendered_view(entity_id, |window| {
                let prepaint_start = window.prepaint_index();
                window.with_element_state::<ViewElementState, _>(
                    global_id.unwrap(),
                    |mut element_state, window| {
                        let content_mask = window.content_mask();
                        let text_style = window.text_style();
                        // 检查器打开时禁用 prepaint 复用，保证树记录完整。
                        // 面板自举期（`inspector` 暂存、`suspended` 置位）同样禁用：
                        // 此时 `is_open` 瞬时为假，不补门会复用过期面板帧。
                        #[cfg(any(feature = "inspector", debug_assertions))]
                        let inspector_reuse_disabled =
                            window.is_inspector_open() || window.inspector_tree_suspended;
                        #[cfg(not(any(feature = "inspector", debug_assertions)))]
                        let inspector_reuse_disabled = false;

                        // Retained P2a：干净视图复用上帧输出，
                        // 无论本帧是否渲染（非缓存视图照常渲染，仅输出被丢弃）。
                        if let Some(state) = element_state.as_mut()
                            && state.cache_key.bounds == bounds
                            && state.cache_key.content_mask == content_mask
                            && state.cache_key.text_style == text_style
                            && !window.dirty_views.contains(&entity_id)
                            && !window.refreshing
                            && !inspector_reuse_disabled
                            // 总开关关闭即全量重建（oracle 对照基线；默认跟随环境变量）。
                            && window.retention_enabled()
                            // 无障碍激活时禁用复用（焦点／树 bookkeeping 在绘制期，跳过即过期）。
                            && !window.a11y.is_active()
                            && !state.dependencies.is_stale(
                                &|id| cx.entity_generation(id),
                                &|global_type| cx.global_generation_by_type(global_type),
                            )
                        {
                            // 丢弃本帧渲染产物（布局树本帧末清空，无残留）。
                            let _ = element.take();
                            self.pending_accessed_entities = None;
                            self.pending_dependencies = None;
                            let mut element_state = element_state.unwrap();
                            window.reuse_prepaint(element_state.prepaint_range.clone());
                            cx.entities
                                .extend_accessed(&element_state.accessed_entities);
                            let prepaint_end = window.prepaint_index();
                            element_state.prepaint_range = prepaint_start..prepaint_end;
                            window.fast_stats.note_view_reused();

                            return (None, element_state);
                        }

                        window.fast_stats.note_view_rebuilt();
                        if let Some(mut element) = element.take() {
                            // 布局阶段已渲染并布局：直接 prepaint，
                            // 并把布局阶段记录与本阶段记录合并后存储。
                            let accessed_entities =
                                self.pending_accessed_entities.take().unwrap_or_default();
                            let mut dependencies =
                                self.pending_dependencies.take().unwrap_or_default();
                            cx.begin_dependency_recording();
                            element.prepaint(window, cx);
                            dependencies.merge(cx.end_dependency_recording());
                            let prepaint_end = window.prepaint_index();
                            let mut element_state =
                                element_state.unwrap_or_else(|| ViewElementState {
                                    accessed_entities: FxHashSet::default(),
                                    dependencies: crate::fast::dependencies::DependencySet::default(
                                    ),
                                    prepaint_range: prepaint_start.clone()..prepaint_end.clone(),
                                    paint_range: PaintIndex::default()..PaintIndex::default(),
                                    cache_key: ViewElementCacheKey {
                                        bounds,
                                        content_mask,
                                        text_style: text_style.clone(),
                                    },
                                });
                            element_state.accessed_entities = accessed_entities;
                            element_state.dependencies = dependencies;
                            element_state.prepaint_range = prepaint_start..prepaint_end;
                            element_state.cache_key = ViewElementCacheKey {
                                bounds,
                                content_mask,
                                text_style,
                            };
                            return (Some(element), element_state);
                        }

                        let refreshing = mem::replace(&mut window.refreshing, true);
                        // Retained P1：绘制前后开关依赖记录。
                        cx.begin_dependency_recording();
                        let (mut element, accessed_entities) = cx.detect_accessed_entities(|cx| {
                            let mut element = self
                                .view
                                .take()
                                .unwrap()
                                .render(window, cx)
                                .into_any_element();
                            element.layout_as_root(bounds.size.into(), window, cx);
                            element.prepaint_at(bounds.origin, window, cx);
                            element
                        });
                        let dependencies = cx.end_dependency_recording();

                        let prepaint_end = window.prepaint_index();
                        window.refreshing = refreshing;

                        (
                            Some(element),
                            ViewElementState {
                                accessed_entities,
                                dependencies,
                                prepaint_range: prepaint_start..prepaint_end,
                                paint_range: PaintIndex::default()..PaintIndex::default(),
                                cache_key: ViewElementCacheKey {
                                    bounds,
                                    content_mask,
                                    text_style,
                                },
                            },
                        )
                    },
                )
            })
        } else {
            // Stateless path: just prepaint the element.
            window.with_id(
                ElementId::Name(std::any::type_name::<V>().into()),
                |window| {
                    element.as_mut().unwrap().prepaint(window, cx);
                },
            );
            Some(element.take().unwrap())
        }
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        element: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(entity_id) = self.entity_id {
            // Stateful path.
            window.with_rendered_view(entity_id, |window| {
                #[cfg(any(feature = "inspector", debug_assertions))]
                let caching_disabled = window.is_inspector_open();
                #[cfg(not(any(feature = "inspector", debug_assertions)))]
                let caching_disabled = false;
                if self.cached_style.is_some() && !caching_disabled {
                    window.with_element_state::<ViewElementState, _>(
                        global_id.unwrap(),
                        |element_state, window| {
                            let mut element_state = element_state.unwrap();

                            let paint_start = window.paint_index();

                            if let Some(element) = element {
                                let refreshing = mem::replace(&mut window.refreshing, true);
                                element.paint(window, cx);
                                window.refreshing = refreshing;
                            } else {
                                window.reuse_paint(element_state.paint_range.clone());
                            }

                            let paint_end = window.paint_index();
                            element_state.paint_range = paint_start..paint_end;

                            ((), element_state)
                        },
                    )
                } else if let Some(mut fresh) = element.take() {
                    // 非缓存视图新鲜绘制：记录 paint 区间并存入元素状态，
                    // 把 paint 期读取并入快照（三阶段记录闭环）。
                    let paint_start = window.paint_index();
                    cx.begin_dependency_recording();
                    fresh.paint(window, cx);
                    let paint_reads = cx.end_dependency_recording();
                    let paint_end = window.paint_index();
                    window.with_element_state::<ViewElementState, _>(
                        global_id.unwrap(),
                        |element_state, _window| {
                            let mut element_state = element_state.unwrap();
                            element_state.paint_range = paint_start..paint_end;
                            element_state.dependencies.merge(paint_reads);
                            (Some(fresh), element_state)
                        },
                    );
                } else {
                    // prepaint 已复用：重放 paint 区间。
                    window.with_element_state::<ViewElementState, _>(
                        global_id.unwrap(),
                        |element_state, window| {
                            let mut element_state = element_state.unwrap();
                            let paint_start = window.paint_index();
                            window.reuse_paint(element_state.paint_range.clone());
                            let paint_end = window.paint_index();
                            element_state.paint_range = paint_start..paint_end;
                            ((), element_state)
                        },
                    );
                }
            });
        } else {
            // Stateless path: just paint the element.
            window.with_id(
                ElementId::Name(std::any::type_name::<V>().into()),
                |window| {
                    element.as_mut().unwrap().paint(window, cx);
                },
            );
        }
    }
}

/// 不渲染任何内容的视图
pub struct EmptyView;

impl Render for EmptyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppContext, ParentElement, SharedString, TestAppContext, div};

    /// 静态探针视图：无实体／全局读取，帧间恒定。
    struct StaticProbe;

    impl Render for StaticProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(SharedString::from("static"))
        }
    }

    /// 无变化第二帧复用视图输出（P2a 验收：复用计数增长见证）。
    #[test]
    fn static_view_reused_across_frames() {
        let mut test_app = TestAppContext::single();
        let window = test_app.add_window(|_, _| StaticProbe);
        let any_window = window.into();

        let draw = |test_app: &mut TestAppContext| {
            test_app
                .update_window(any_window, |_, window, cx| {
                    window.draw(cx).clear(cx);
                    window.fast_stats.snapshot()
                })
                .unwrap()
        };

        let first = draw(&mut test_app);
        assert!(first.views_rebuilt > 0);
        let second = draw(&mut test_app);
        // 静态帧必有复用（窗口创建时可能已绘制过首帧，故不断言绝对值，只看增量）。
        assert!(second.views_reused > first.views_reused);
    }
}
