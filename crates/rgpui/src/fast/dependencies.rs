//! 依赖追踪：记录视图读过什么、判定何时过期（P1）。
//!
//! 对标 `gpui-fast` 的 `fast/dependencies.rs`，但 generation 存放在数据属主处
//! （实体 → [`EntityMap`](crate::app::EntityMap)，全局 → [`App`](crate::App)），
//! 本模块只提供版本号类型、记录器与过期判定，不持有任何应用状态。
//!
//! 语义（与后续 P2 复用直接对应）：
//! - 实体：`read` 记录 `(id, generation)`；任何 `update`（`lease` 路径）递增，
//!   绘制内外一致；视图不记录自身（渲染自增殖每帧可见会恒脏，靠 `notify` 链），
//!   记录后 generation 变化即过期；
//! - 全局：`global` / `try_global` 记录 `(type, generation)`；`has_global`
//!   只记录存在性（存在与否变化才过期，值写入不影响）；
//! - 任何写路径（`global_mut` / `set_global` / `remove_global` / 租借归还）递增。

use crate::{EntityId, collections::FxHashSet};
use std::{any::TypeId, cell::Cell, rc::Rc};

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
    /// 读过的实体外共享状态（滚动句柄、列表状态）及其当时的版本号。
    states: Vec<(StateVersion, u64)>,
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

    /// 记录一次实体外共享状态读取。
    pub(crate) fn record_state(&mut self, version: &StateVersion) {
        self.states.push((version.clone(), version.get()));
    }

    /// 并入子快照的实体部分（父视图复用判定须包含嵌套视图的读取，P2a）。
    ///
    /// 重复用哈希定位：嵌套视图的读取包含其所有后代，逐条回溯是二次方。
    /// 完全相同的（实体，版本号）只保留首次读取；同一实体的不同版本号读取
    /// 都保留——输出可能依赖任一次读取，`is_stale` 按任一条不一致即过期，
    /// 语义与不去重时一致（重复条目本来就不影响 `any` 判定）。
    pub(crate) fn absorb_entities(&mut self, set: &DependencySet) {
        let mut seen = FxHashSet::with_capacity_and_hasher(
            self.entities.len() + set.entities.len(),
            Default::default(),
        );
        self.entities.retain(|read| seen.insert(*read));
        for read in set.entities.iter().copied() {
            if seen.insert(read) {
                self.entities.push(read);
            }
        }
    }

    /// 并入子快照的全局与共享状态部分（同上）。
    ///
    /// 去重口径与 [`Self::absorb_entities`] 一致：全局按（类型，读取方式）
    /// 整体判等——同一全局以不同方式读取（值读取 / 存在性检查）视为不同依赖，
    /// 都保留；共享状态按（计数器标识，读取时版本号）判等——同一状态读到
    /// 不同版本号都保留。
    pub(crate) fn absorb_shared(&mut self, set: &DependencySet) {
        let mut seen = FxHashSet::with_capacity_and_hasher(
            self.globals.len() + set.globals.len(),
            Default::default(),
        );
        self.globals.retain(|read| seen.insert(*read));
        for read in set.globals.iter().cloned() {
            if seen.insert(read) {
                self.globals.push(read);
            }
        }

        let mut seen = FxHashSet::with_capacity_and_hasher(
            self.states.len() + set.states.len(),
            Default::default(),
        );
        self.states
            .retain(|(version, read_at)| seen.insert((version.id(), *read_at)));
        for (version, read_at) in set.states.iter().cloned() {
            if seen.insert((version.id(), read_at)) {
                self.states.push((version, read_at));
            }
        }
    }

    /// 结束记录，生成不可变的依赖快照。
    pub(crate) fn finish(self) -> DependencySet {
        DependencySet {
            entities: self.entities,
            globals: self.globals,
            states: self.states,
        }
    }
}

/// 实体外共享状态的版本号：滚动句柄、列表状态等。
///
/// `Rc` 共享保证句柄克隆看到同一计数器；`Cell` 允许无 `App` 上下文的
/// 读路径（如 `ScrollHandle::offset`）同样打戳而不引入借用分歧。
/// 递增即“渲染结果可能变化”，只减不增的刷新（如每帧重建子边界）不得递增。
#[derive(Clone, Default, Debug)]
pub(crate) struct StateVersion(Rc<Cell<u64>>);

impl StateVersion {
    /// 读取当前版本号。
    pub(crate) fn get(&self) -> u64 {
        self.0.get()
    }

    /// 标记状态已变化。
    pub(crate) fn bump(&self) {
        self.0.set(self.0.get().wrapping_add(1));
    }

    /// 可能未实际变化的写入：变化才递增（如 `set_offset` 写入相同值）。
    pub(crate) fn bump_if(&self, changed: bool) {
        if changed {
            self.bump();
        }
    }

    /// 状态计数器的稳定标识（哈希去重用；克隆共享同一 `Rc`，标识相同）。
    pub(crate) fn id(&self) -> usize {
        Rc::as_ptr(&self.0) as *const u8 as usize
    }
}

/// 全局读取方式：值读取关心版本号，存在性检查只关心有无。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// 绘制时读过的实体外共享状态及其版本号（`Rc` 共享，查询无需外部表）。
    states: Vec<(StateVersion, u64)>,
}

impl DependencySet {
    /// 测试用：共享状态读取清单。
    #[cfg(test)]
    pub(crate) fn state_reads(&self) -> &[(StateVersion, u64)] {
        &self.states
    }

    /// 测试用：实体读取清单。
    #[cfg(test)]
    pub(crate) fn entity_reads(&self) -> &[(EntityId, Generation)] {
        &self.entities
    }

    /// 合并另一快照（实体、全局与共享状态记录器分别收集后汇总）。
    ///
    /// 去重口径与 [`DependencyRecorder::absorb_entities`] 一致：完全相同的
    /// 读取只保留首次（哈希定位），语义不变。
    pub(crate) fn merge(&mut self, other: DependencySet) {
        let mut seen = FxHashSet::with_capacity_and_hasher(
            self.entities.len() + other.entities.len(),
            Default::default(),
        );
        self.entities.retain(|read| seen.insert(*read));
        for read in other.entities {
            if seen.insert(read) {
                self.entities.push(read);
            }
        }

        let mut seen = FxHashSet::with_capacity_and_hasher(
            self.globals.len() + other.globals.len(),
            Default::default(),
        );
        self.globals.retain(|read| seen.insert(*read));
        for read in other.globals {
            if seen.insert(read) {
                self.globals.push(read);
            }
        }

        let mut seen = FxHashSet::with_capacity_and_hasher(
            self.states.len() + other.states.len(),
            Default::default(),
        );
        self.states
            .retain(|(version, read_at)| seen.insert((version.id(), *read_at)));
        for (version, read_at) in other.states {
            if seen.insert((version.id(), read_at)) {
                self.states.push((version, read_at));
            }
        }
    }

    /// 判定快照是否过期。
    ///
    /// - 实体：当前版本号与记录不一致即过期；实体已消失（查不到版本号）
    ///   按过期处理（视图持有失效实体本就该重建）；
    /// - 全局值读取：版本号不一致或全局已消失即过期；
    /// - 存在性记录：存在与否翻转才过期，值写入不影响；
    /// - 共享状态：计数器变化即过期（自包含查询，无需外部表）。
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
            || self
                .states
                .iter()
                .any(|(version, recorded)| version.get() != *recorded)
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

    /// 共享状态：递增即过期；`bump_if(false)` 保持新鲜；克隆句柄共享计数器。
    #[test]
    fn state_version_stales_on_bump() {
        let version = StateVersion::default();
        let alias = version.clone();
        assert_eq!(version.get(), alias.get());

        let mut recorder = DependencyRecorder::new();
        recorder.record_state(&version);
        let snapshot = recorder.finish();

        let no_entities = |_: EntityId| None;
        let no_globals = |_: TypeId| None;
        assert!(!snapshot.is_stale(&no_entities, &no_globals));

        version.bump_if(false);
        assert!(!snapshot.is_stale(&no_entities, &no_globals));

        alias.bump();
        assert!(snapshot.is_stale(&no_entities, &no_globals));
    }

    /// 哈希去重（对齐上游 `0f5bf7f`）：完全相同的读取只保留首次、按首次读取
    /// 顺序排列；同一状态 / 全局以不同版本号或读取方式再读则都保留。
    #[test]
    fn merge_dedupes_repeats_keeping_first_read_in_order() {
        let (a, b, c) = (
            StateVersion::default(),
            StateVersion::default(),
            StateVersion::default(),
        );
        // 首次读取顺序与创建顺序不同；随后 a、c 各变一次，再混入重复读取。
        let mut first = DependencyRecorder::new();
        first.record_state(&c);
        first.record_state(&a);
        let mut second = DependencyRecorder::new();
        a.bump();
        c.bump();
        second.record_state(&a);
        second.record_state(&b);
        second.record_state(&c);
        second.record_state(&a);

        let mut merged = first.finish();
        merged.merge(second.finish());

        let states: Vec<(usize, u64)> = merged
            .state_reads()
            .iter()
            .map(|(version, read_at)| (version.id(), *read_at))
            .collect();
        // 按出现顺序排列（先 self 后 other 各自保序）；a、c 在两次记录之间
        // 变过版本，两个版本号的读取都保留（保守语义：输出可能依赖任一次读取）。
        assert_eq!(
            states,
            vec![
                (c.id(), 0),
                (a.id(), 0),
                (a.id(), a.get()),
                (b.id(), 0),
                (c.id(), c.get()),
            ]
        );
        // 完全相同的重复已去掉：a 除首读与变版后再读外没有多余条目。
        let a_reads = states.iter().filter(|(id, _)| *id == a.id()).count();
        let c_reads = states.iter().filter(|(id, _)| *id == c.id()).count();
        assert_eq!(a_reads, 2, "同状态不同版本号的读取都应保留");
        assert_eq!(c_reads, 2, "同状态不同版本号的读取都应保留");
    }

    /// 同一全局以不同方式读取（值读取 / 存在性检查）视为不同依赖：
    /// 完全相同的重复才去重，is_stale 语义不变。
    #[test]
    fn merge_keeps_different_global_read_kinds() {
        let global_type = TypeId::of::<u32>();
        let mut first = DependencyRecorder::new();
        first.record_global(global_type, Generation::initial());
        first.record_entity(EntityId::from(1u64), Generation::initial());
        let mut second = DependencyRecorder::new();
        second.record_entity(EntityId::from(1u64), Generation::initial());
        second.record_global_presence(global_type, true);
        second.record_entity(EntityId::from(2u64), Generation::initial());

        let mut merged = first.finish();
        merged.merge(second.finish());

        // 值读取 + 存在性读取并存：值写入或全局移除都应过期；全局保持原值
        // 且实体都新鲜时不过期。
        let entities_fresh = |_: EntityId| Some(Generation::initial());
        let entities_gone = |_: EntityId| None;
        assert!(merged.is_stale(&entities_fresh, &|t: TypeId| {
            if t == global_type {
                Some(Generation::initial().bumped())
            } else {
                None
            }
        }));
        assert!(merged.is_stale(&entities_gone, &|t: TypeId| {
            if t == global_type {
                Some(Generation::initial())
            } else {
                None
            }
        }));
        assert!(!merged.is_stale(&entities_fresh, &|t: TypeId| {
            if t == global_type {
                Some(Generation::initial())
            } else {
                None
            }
        }));
        // 实体：完全相同的 (id, generation) 去重，新实体保留。
        assert_eq!(merged.entity_reads().len(), 2);
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
