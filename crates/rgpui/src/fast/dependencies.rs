//! 依赖追踪：记录视图读过什么、判定何时过期（P1）。
//!
//! 对标 `gpui-fast` 的 `fast/dependencies.rs`，但 generation 存放在数据属主处
//! （实体 → [`EntityMap`](crate::app::EntityMap)，全局 → [`App`](crate::App)），
//! 本模块只提供版本号类型、记录器与过期判定，不持有任何应用状态。
//!
//! 语义（与后续 P2 复用直接对应）：
//! - 实体：`read` 记录 `(id, generation)`；`update`（`lease` 路径）递增；
//!   记录后 generation 变化即过期；
//! - 全局：`global` / `try_global` 记录 `(type, generation)`；`has_global`
//!   只记录存在性（存在与否变化才过期，值写入不影响）；
//! - 任何写路径（`global_mut` / `set_global` / `remove_global` / 租借归还）递增。

use crate::EntityId;
use std::any::TypeId;

/// 单调递增的版本号：每次“可能改变渲染结果”的写入递增一次。
///
/// 从 0 开始；`wrapping_add` 回绕在实验分支可接受（2^64 次写入才回绕，
/// 且回绕只会导致一次应脏未脏，后续写入即恢复正确）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Generation(u64);

impl Generation {
    /// 初始版本号（实体创建、全局首次写入时）。
    pub(crate) fn initial() -> Self {
        Self(0)
    }

    /// 递增并返回新版本号。
    pub(crate) fn bumped(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

/// 一次记录过程收集的读集（单个视图一次绘制的依赖）。
#[derive(Debug, Default)]
pub(crate) struct DependencyRecorder {
    /// 读过的实体及其当时的版本号。
    entities: Vec<(EntityId, Generation)>,
    /// 读过的全局及其读取方式。
    globals: Vec<(TypeId, GlobalRead)>,
}

impl DependencyRecorder {
    /// 创建空记录器。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 记录一次实体读取。
    pub(crate) fn record_entity(&mut self, id: EntityId, generation: Generation) {
        self.entities.push((id, generation));
    }

    /// 记录一次全局值读取。
    pub(crate) fn record_global(&mut self, global_type: TypeId, generation: Generation) {
        self.globals
            .push((global_type, GlobalRead::Value(generation)));
    }

    /// 记录一次 `has_global` 存在性检查。
    pub(crate) fn record_global_presence(&mut self, global_type: TypeId, present: bool) {
        let read = if present {
            GlobalRead::Present
        } else {
            GlobalRead::Absent
        };
        self.globals.push((global_type, read));
    }

    /// 结束记录，生成不可变的依赖快照。
    pub(crate) fn finish(self) -> DependencySet {
        DependencySet {
            entities: self.entities,
            globals: self.globals,
        }
    }
}

/// 全局读取方式：值读取关心版本号，存在性检查只关心有无。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlobalRead {
    /// 值读取时的版本号。
    Value(Generation),
    /// `has_global` 为真。
    Present,
    /// `has_global` 为假。
    Absent,
}

/// 一次绘制记录的依赖快照：P2 判定视图能否复用的依据。
#[derive(Debug, Default, Clone)]
pub(crate) struct DependencySet {
    /// 绘制时读过的实体及其版本号。
    entities: Vec<(EntityId, Generation)>,
    /// 绘制时读过的全局及其读取方式。
    globals: Vec<(TypeId, GlobalRead)>,
}

impl DependencySet {
    /// 合并另一快照（实体与全局记录器分别收集后汇总）。
    pub(crate) fn merge(&mut self, other: DependencySet) {
        self.entities.extend(other.entities);
        self.globals.extend(other.globals);
    }

    /// 判定快照是否过期。
    ///
    /// - 实体：当前版本号与记录不一致即过期；实体已消失（查不到版本号）
    ///   按过期处理（视图持有失效实体本就该重建）；
    /// - 全局值读取：版本号不一致或全局已消失即过期；
    /// - 存在性记录：存在与否翻转才过期，值写入不影响。
    pub(crate) fn is_stale(
        &self,
        entity_generation: &impl Fn(EntityId) -> Option<Generation>,
        global_generation: &impl Fn(TypeId) -> Option<Generation>,
    ) -> bool {
        self.entities
            .iter()
            .any(|(id, recorded)| entity_generation(*id).is_none_or(|current| current != *recorded))
            || self.globals.iter().any(|(global_type, read)| {
                let current = global_generation(*global_type);
                match read {
                    GlobalRead::Value(recorded) => {
                        current.is_none_or(|generation| generation != *recorded)
                    }
                    GlobalRead::Present => current.is_none(),
                    GlobalRead::Absent => current.is_some(),
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 实体写入后记录快照过期，未写则新鲜。
    #[test]
    fn entity_write_stales_snapshot() {
        let id = EntityId::from(7u64);
        let mut recorder = DependencyRecorder::new();
        recorder.record_entity(id, Generation::initial());
        let snapshot = recorder.finish();

        let unchanged = |_: EntityId| Some(Generation::initial());
        let changed = |_: EntityId| Some(Generation::initial().bumped());
        let gone = |_: EntityId| None;
        let no_globals = |_: TypeId| None;
        assert!(!snapshot.is_stale(&unchanged, &no_globals));
        assert!(snapshot.is_stale(&changed, &no_globals));
        assert!(snapshot.is_stale(&gone, &no_globals));
    }

    /// 全局值写入使值读取过期，但不影响纯存在性记录；存在性翻转使存在性记录过期。
    #[test]
    fn global_presence_semantics() {
        let global_type = TypeId::of::<u32>();
        let mut recorder = DependencyRecorder::new();
        recorder.record_global(global_type, Generation::initial());
        let value_snapshot = recorder.finish();

        let mut recorder = DependencyRecorder::new();
        recorder.record_global_presence(global_type, true);
        let presence_snapshot = recorder.finish();

        let mut recorder = DependencyRecorder::new();
        recorder.record_global_presence(global_type, false);
        let absence_snapshot = recorder.finish();

        let written = |_: TypeId| Some(Generation::initial().bumped());
        let same = |_: TypeId| Some(Generation::initial());
        let removed = |_: TypeId| None;
        let no_entities = |_: EntityId| None;

        assert!(value_snapshot.is_stale(&no_entities, &written));
        assert!(!value_snapshot.is_stale(&no_entities, &same));
        assert!(value_snapshot.is_stale(&no_entities, &removed));
        // 纯存在性记录：值写入不影响，移除才过期。
        assert!(!presence_snapshot.is_stale(&no_entities, &written));
        assert!(!presence_snapshot.is_stale(&no_entities, &same));
        assert!(presence_snapshot.is_stale(&no_entities, &removed));
        // 不存在记录：出现即过期。
        assert!(absence_snapshot.is_stale(&no_entities, &written));
        assert!(absence_snapshot.is_stale(&no_entities, &same));
        assert!(!absence_snapshot.is_stale(&no_entities, &removed));
    }

    /// 合并逻辑：双方记录并存。
    #[test]
    fn merge_keeps_both_sides() {
        let no_entities = |_: EntityId| None;
        let no_globals = |_: TypeId| None;

        let id = EntityId::from(1u64);
        let mut recorder = DependencyRecorder::new();
        recorder.record_entity(id, Generation::initial());
        let mut merged = DependencySet::default();
        merged.merge(recorder.finish());
        assert!(merged.is_stale(&no_entities, &no_globals));
    }

    /// 真实 `App` 路径：记录读集 → 无写入新鲜 → 实体/全局写入分别过期。
    #[test]
    fn app_paths_record_and_bump_generations() {
        use crate::{Global, TestApp};

        struct Probe(u32);
        impl Global for Probe {}

        let mut app = TestApp::new();
        let entity = app.new_entity(|_| 41u32);
        app.set_global(Probe(1));

        // 记录一次“绘制”：读实体 + 读全局。
        let snapshot = app.update(|cx| {
            cx.begin_dependency_recording();
            assert_eq!(*entity.read(cx), 41);
            assert_eq!(cx.global::<Probe>().0, 1);
            cx.end_dependency_recording()
        });

        // 无写入：快照新鲜。
        app.read(|cx| {
            assert!(!snapshot.is_stale(&|id| cx.entity_generation(id), &|ty| {
                cx.global_generation_by_type(ty)
            }));
        });

        // 实体更新：快照过期。
        app.update_entity(&entity, |value, _| *value += 1);
        app.read(|cx| {
            assert!(snapshot.is_stale(&|id| cx.entity_generation(id), &|ty| {
                cx.global_generation_by_type(ty)
            }));
        });

        // 全局写入同样使新快照过期（重新记录后再写）。
        let snapshot = app.update(|cx| {
            cx.begin_dependency_recording();
            assert_eq!(*entity.read(cx), 42);
            assert_eq!(cx.global::<Probe>().0, 1);
            cx.end_dependency_recording()
        });
        app.update_global::<Probe, _>(|probe, _| probe.0 = 2);
        app.read(|cx| {
            assert!(snapshot.is_stale(&|id| cx.entity_generation(id), &|ty| {
                cx.global_generation_by_type(ty)
            }));
        });
    }
}
