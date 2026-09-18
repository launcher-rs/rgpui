//! 检查器 —— 提供开发者调试工具，用于标识和检查视图元素树。

/// 可检查元素的唯一标识符。
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct InspectorElementId {
    /// ID 的稳定部分。
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub path: std::rc::Rc<InspectorElementPath>,
    /// 区分具有相同路径的元素。
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub instance_id: usize,
}

impl Into<InspectorElementId> for &InspectorElementId {
    fn into(self) -> InspectorElementId {
        self.clone()
    }
}

/// 检查器元素 id 的面板展示 helpers（I2 完整树行标签/展开键）。
#[cfg(any(feature = "inspector", debug_assertions))]
impl InspectorElementId {
    /// 行标签：全局路径末段 id；匿名元素退回源码文件名。
    pub fn short_label(&self) -> String {
        if let Some(last) = self.path.global_id.0.last() {
            last.to_string()
        } else {
            self.path.source_location.file().to_string()
        }
    }

    /// 源码位置标签（`文件:行`）。
    pub fn source_label(&self) -> String {
        let loc = self.path.source_location;
        format!("{}:{}", loc.file(), loc.line())
    }

    /// 跨帧稳定键：全局路径 + 实例号 + 源码位置（展开/折叠状态用）。
    pub fn tree_key(&self) -> String {
        let loc = self.path.source_location;
        format!(
            "{}#{}@{}:{}",
            self.path.global_id,
            self.instance_id,
            loc.file(),
            loc.line()
        )
    }
}

#[cfg(any(feature = "inspector", debug_assertions))]
pub use conditional::*;

#[cfg(any(feature = "inspector", debug_assertions))]
mod conditional {
    use super::*;
    use crate::collections::{FxHashMap, TypeIdHashMap};
    use crate::{AnyElement, App, Bounds, Context, Empty, IntoElement, Pixels, Render, Window};
    use std::any::{Any, TypeId};

    /// 由元素构造源位置限定的 `GlobalElementId`。
    #[derive(Debug, Eq, PartialEq, Hash)]
    pub struct InspectorElementPath {
        /// 到具有 `ElementId` 的最近祖先元素的路径。
        #[cfg(any(feature = "inspector", debug_assertions))]
        pub global_id: crate::GlobalElementId,
        /// 构造此元素的源位置。
        #[cfg(any(feature = "inspector", debug_assertions))]
        pub source_location: &'static std::panic::Location<'static>,
    }

    impl Clone for InspectorElementPath {
        fn clone(&self) -> Self {
            Self {
                global_id: self.global_id.clone(),
                source_location: self.source_location,
            }
        }
    }

    impl Into<InspectorElementPath> for &InspectorElementPath {
        fn into(self) -> InspectorElementPath {
            self.clone()
        }
    }

    /// 在 `App` 上设置的用于渲染检查器 UI 的函数。
    pub type InspectorRenderer =
        Box<dyn Fn(&mut Inspector, &mut Window, &mut Context<Inspector>) -> AnyElement>;

    /// 管理检查器状态 - 当前选中的元素以及检查器是否处于
    /// 拾取模式。
    pub struct Inspector {
        active_element: Option<InspectedElement>,
        pub(crate) pick_depth: Option<f32>,
    }

    struct InspectedElement {
        id: InspectorElementId,
        states: TypeIdHashMap<Box<dyn Any>>,
    }

    impl InspectedElement {
        fn new(id: InspectorElementId) -> Self {
            InspectedElement {
                id,
                states: Default::default(),
            }
        }
    }

    impl Inspector {
        pub(crate) fn new() -> Self {
            Self {
                active_element: None,
                pick_depth: Some(0.0),
            }
        }

        /// 选中指定元素并退出拾取模式。
        ///
        /// 公开给检查器面板调用（如树节点点击选中画布对应区域）；
        /// 拾取点击路径内部同样复用此方法。
        pub fn select(&mut self, id: InspectorElementId, window: &mut Window) {
            self.set_active_element_id(id, window);
            self.pick_depth = None;
        }

        /// 按祖先层级上移选中（I1 双向映射）。
        ///
        /// `levels_up` 为从当前选中元素沿全局路径上移的层数：
        /// `0` 表示保持当前选中，`1` 为父级，依此类推。
        /// 在当前帧 `inspector_hitboxes` 注册表中按全局路径前缀反查祖先 id，
        /// 以 hitbox 包含关系消歧同路径多实例（取包含当前选中区域的最小祖先边界）。
        /// 成功时复用拾取高亮绘制选中区域，返回 `true`；无选中或越界返回 `false`。
        pub fn select_ancestor(&mut self, levels_up: usize, window: &mut Window) -> bool {
            let Some(active_id) = self.active_element_id().cloned() else {
                return false;
            };
            if levels_up == 0 {
                return true;
            }
            let active_global = active_id.path.global_id.clone();
            let active_len = active_global.0.len();
            if levels_up > active_len {
                return false;
            }
            let target_len = active_len - levels_up;
            let target_prefix = &active_global.0[..target_len];

            // 当前选中区域（优先已渲染帧，退回正在绘制帧），用于包含消歧。
            let active_bounds = window
                .inspector_bounds_for_id(&active_id)
                .or_else(|| window.next_inspector_bounds_for_id(&active_id));

            // 收集全局路径与目标前缀精确匹配的候选祖先。
            let mut candidates: Vec<(InspectorElementId, crate::Bounds<crate::Pixels>)> =
                Vec::new();
            for frame in [&window.rendered_frame, &window.next_frame] {
                for (hitbox_id, inspector_id) in frame.inspector_hitboxes.iter() {
                    if inspector_id.path.global_id.0.as_ref() != target_prefix {
                        continue;
                    }
                    if let Some(hitbox) =
                        frame.hitboxes.iter().find(|hitbox| hitbox.id == *hitbox_id)
                    {
                        candidates.push((inspector_id.clone(), hitbox.bounds));
                    }
                }
            }
            if candidates.is_empty() {
                return false;
            }

            // 以包含关系消歧：取包含当前选中区域的最小祖先边界；
            // 无选中区域或无包含者时退回首个候选。
            let chosen = if let Some(active_bounds) = active_bounds.as_ref() {
                candidates
                    .iter()
                    .filter(|(_, bounds)| bounds_contains(bounds, active_bounds))
                    .min_by(|a, b| {
                        bounds_area(&a.1)
                            .partial_cmp(&bounds_area(&b.1))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(id, _)| id.clone())
            } else {
                None
            };
            let chosen = chosen.unwrap_or_else(|| {
                // 去重后取首个（同一帧可能在 rendered/next 重复出现）。
                let mut seen = std::collections::HashSet::new();
                candidates
                    .into_iter()
                    .map(|(id, _)| id)
                    .find(|id| seen.insert(id.clone()))
                    .expect("候选非空")
            });

            self.select(chosen, window);
            true
        }

        pub(crate) fn hover(&mut self, id: InspectorElementId, window: &mut Window) {
            if self.is_picking() {
                let changed = self.set_active_element_id(id, window);
                if changed {
                    self.pick_depth = Some(0.0);
                }
            }
        }

        pub(crate) fn set_active_element_id(
            &mut self,
            id: InspectorElementId,
            window: &mut Window,
        ) -> bool {
            let changed = Some(&id) != self.active_element_id();
            if changed {
                self.active_element = Some(InspectedElement::new(id));
                window.refresh();
            }
            changed
        }

        /// 当前悬停或选中元素的 ID。
        pub fn active_element_id(&self) -> Option<&InspectorElementId> {
            self.active_element.as_ref().map(|e| &e.id)
        }

        pub(crate) fn with_active_element_state<T: 'static, R>(
            &mut self,
            window: &mut Window,
            f: impl FnOnce(&mut Option<T>, &mut Window) -> R,
        ) -> R {
            let Some(active_element) = &mut self.active_element else {
                return f(&mut None, window);
            };

            let type_id = TypeId::of::<T>();
            let mut inspector_state = active_element
                .states
                .remove(&type_id)
                .map(|state| *state.downcast().unwrap());

            let result = f(&mut inspector_state, window);

            if let Some(inspector_state) = inspector_state {
                active_element
                    .states
                    .insert(type_id, Box::new(inspector_state));
            }

            result
        }

        /// 启动元素拾取模式，允许用户通过点击选择元素。
        pub fn start_picking(&mut self) {
            self.pick_depth = Some(0.0);
        }

        /// 返回检查器当前是否处于拾取模式。
        pub fn is_picking(&self) -> bool {
            self.pick_depth.is_some()
        }

        /// 为活动检查器元素的所有已注册检查器状态渲染元素。
        pub fn render_inspector_states(
            &mut self,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) -> Vec<AnyElement> {
            let mut elements = Vec::new();
            if let Some(active_element) = self.active_element.take() {
                for (type_id, state) in &active_element.states {
                    if let Some(render_inspector) = cx
                        .inspector_element_registry
                        .renderers_by_type_id
                        .remove(type_id)
                    {
                        let mut element = (render_inspector)(
                            active_element.id.clone(),
                            state.as_ref(),
                            window,
                            cx,
                        );
                        elements.push(element);
                        cx.inspector_element_registry
                            .renderers_by_type_id
                            .insert(*type_id, render_inspector);
                    }
                }

                self.active_element = Some(active_element);
            }

            elements
        }
    }

    impl Render for Inspector {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if let Some(inspector_renderer) = cx.inspector_renderer.take() {
                let result = inspector_renderer(self, window, cx);
                cx.inspector_renderer = Some(inspector_renderer);
                result
            } else {
                Empty.into_any_element()
            }
        }
    }

    #[derive(Default)]
    pub(crate) struct InspectorElementRegistry {
        renderers_by_type_id: FxHashMap<
            TypeId,
            Box<dyn Fn(InspectorElementId, &dyn Any, &mut Window, &mut App) -> AnyElement>,
        >,
    }

    impl InspectorElementRegistry {
        pub fn register<T: 'static, R: IntoElement>(
            &mut self,
            f: impl 'static + Fn(InspectorElementId, &T, &mut Window, &mut App) -> R,
        ) {
            self.renderers_by_type_id.insert(
                TypeId::of::<T>(),
                Box::new(move |id, value, window, cx| {
                    let value = value.downcast_ref().unwrap();
                    f(id, value, window, cx).into_any_element()
                }),
            );
        }
    }

    /// 检查器元素树节点（I2 完整树）。
    ///
    /// prepaint 期按实际嵌套记录 parent→children，批量大时面板侧复用 `VirtualList`；
    /// 检查器关闭时不记录、不存储，零额外开销。
    #[derive(Debug, Clone, Default)]
    pub(crate) struct InspectorTreeNode {
        /// 父节点（根为 `None`，deferred/overlay 挂为独立根）。
        pub(crate) parent: Option<InspectorElementId>,
        /// 子节点（绘制顺序）。
        pub(crate) children: Vec<InspectorElementId>,
    }

    /// 崩溃快照里的单个节点（可序列化，事后回放/排查用）。
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct SnapshotNode {
        /// 跨帧稳定键（同面板展开键）。
        pub key: String,
        /// 行标签（全局路径末段）。
        pub label: String,
        /// 源码位置（`文件:行`）。
        pub source: String,
        /// 实例号。
        pub instance: usize,
        /// 父节点键（根为 `None`）。
        pub parent: Option<String>,
        /// 边界（x/y/w/h 逻辑像素，无 hitbox 时为 `None`）。
        pub bounds: Option<[f32; 4]>,
    }

    /// 检查器崩溃快照（滚动写入 `last.json`，死后排查用）。
    ///
    /// 由 [`crate::Window::capture_inspector_snapshot`] 采集，
    /// `App::enable_crash_recorder` 开启后约 2 秒一写（原子替换），
    /// 配合 [`crate::runtime_stats::install_crash_hook`] 的 panic 日志食用。
    /// 注意快照随检查器门控：release 未开 `inspector` feature 时无此数据，
    /// 此时仅 panic 日志可用。
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct InspectorSnapshot {
        /// 快照格式版本（当前 1）。
        pub version: u32,
        /// 采集时间（UNIX 毫秒）。
        pub timestamp_millis: u64,
        /// 当前选中。
        pub active: Option<SnapshotNode>,
        /// 选中祖先链（根在前）。
        pub ancestors: Vec<SnapshotNode>,
        /// 全树节点总数（`tree` 可能因截断少于此数）。
        pub tree_total: usize,
        /// 全树扁平节点（按键排序，截断 2000）。
        pub tree: Vec<SnapshotNode>,
        /// 错误环（序号升序）。
        pub errors: Vec<(u64, String)>,
        /// 视口尺寸（w/h 逻辑像素）。
        pub viewport: [f32; 2],
    }

    /// 判断外层边界是否包含内层边界（含相等，允许 1px 舍入误差）。
    fn bounds_contains(outer: &Bounds<Pixels>, inner: &Bounds<Pixels>) -> bool {
        const EPS: f32 = 1.0;
        let outer_right = outer.origin.x.as_f32() + outer.size.width.as_f32();
        let outer_bottom = outer.origin.y.as_f32() + outer.size.height.as_f32();
        let inner_right = inner.origin.x.as_f32() + inner.size.width.as_f32();
        let inner_bottom = inner.origin.y.as_f32() + inner.size.height.as_f32();
        outer.origin.x.as_f32() <= inner.origin.x.as_f32() + EPS
            && outer.origin.y.as_f32() <= inner.origin.y.as_f32() + EPS
            && outer_right + EPS >= inner_right
            && outer_bottom + EPS >= inner_bottom
    }

    /// 边界面积（用于包含消歧时取最小祖先）。
    fn bounds_area(bounds: &Bounds<Pixels>) -> f32 {
        bounds.size.width.as_f32() * bounds.size.height.as_f32()
    }

    /// 快照采集冒烟测试（打开检查器 → 绘制 → 快照非空）。
    #[crate::test]
    fn capture_snapshot_smoke(cx: &mut crate::TestAppContext) {
        use crate::{Context, IntoElement, Render, div};

        struct Probe;
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
            }
        }

        let (_view, cx) = cx.add_window_view(|_, _| Probe);
        cx.update(|window, cx| {
            window.toggle_inspector(cx);
            let _ = window.draw(cx);
            let snapshot = window
                .capture_inspector_snapshot(cx)
                .expect("检查器打开应有快照");
            assert_eq!(snapshot.version, 1);
            assert!(snapshot.viewport[0] > 0.0);
        });
    }

    /// 快照 JSON 形状回归测试（字段改名即炸，提醒同步回放侧）。
    #[test]
    fn snapshot_serializes_stable_shape() {
        let snapshot = InspectorSnapshot {
            version: 1,
            timestamp_millis: 0,
            active: Some(SnapshotNode {
                key: "k".to_string(),
                label: "l".to_string(),
                source: "s:1".to_string(),
                instance: 0,
                parent: None,
                bounds: Some([0.0, 0.0, 10.0, 10.0]),
            }),
            ancestors: Vec::new(),
            tree_total: 1,
            tree: Vec::new(),
            errors: vec![(0, "boom".to_string())],
            viewport: [800.0, 600.0],
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        for key in [
            "version",
            "timestamp_millis",
            "active",
            "ancestors",
            "tree_total",
            "tree",
            "errors",
            "viewport",
            "bounds",
            "source",
        ] {
            assert!(json.contains(key), "快照缺字段 {key}");
        }
    }
}

/// 提供 `#[derive_inspector_reflection]` 使用的定义。
#[cfg(any(feature = "inspector", debug_assertions))]
pub mod inspector_reflection {
    use std::any::Any;

    /// 具有签名 `fn some_fn(T) -> T` 的函数的具化。提供名称、
    /// 文档和调用函数的能力。
    #[derive(Clone, Copy)]
    pub struct FunctionReflection<T> {
        /// 函数的名称
        pub name: &'static str,
        /// 方法
        pub function: fn(Box<dyn Any>) -> Box<dyn Any>,
        /// 函数的文档
        pub documentation: Option<&'static str>,
        /// 参数和结果类型的 `PhantomData`
        pub _type: std::marker::PhantomData<T>,
    }

    impl<T: 'static> FunctionReflection<T> {
        /// 在值上调用此方法并返回结果。
        pub fn invoke(&self, value: T) -> T {
            let boxed = Box::new(value) as Box<dyn Any>;
            let result = (self.function)(boxed);
            *result
                .downcast::<T>()
                .expect("Type mismatch in reflection invoke")
        }
    }
}
