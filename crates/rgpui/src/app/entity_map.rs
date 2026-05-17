use crate::collections::FxHashSet;
use crate::{App, AppContext, GpuiBorrow, VisualContext, Window, seal::Sealed};
use anyhow::{Context as _, Result};
use derive_more::{Deref, DerefMut};
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use slotmap::{KeyData, SecondaryMap, SlotMap};
use std::{
    any::{Any, TypeId, type_name},
    cell::RefCell,
    cmp::Ordering,
    fmt::{self, Display},
    hash::{Hash, Hasher},
    marker::PhantomData,
    num::NonZeroU64,
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, AtomicUsize, Ordering::SeqCst},
    },
    thread::panicking,
};

use super::Context;
#[cfg(any(test, feature = "leak-detection"))]
use crate::collections::HashMap;
use crate::util::atomic_incr_if_not_zero;

slotmap::new_key_type! {
    /// A unique identifier for a entity across the application.
    pub struct EntityId;
}

impl From<u64> for EntityId {
    fn from(value: u64) -> Self {
        Self(KeyData::from_ffi(value))
    }
}

impl EntityId {
    /// Converts this entity id to a [NonZeroU64]
    pub fn as_non_zero_u64(self) -> NonZeroU64 {
        NonZeroU64::new(self.0.as_ffi()).unwrap()
    }

    /// Converts this entity id to a [u64]
    pub fn as_u64(self) -> u64 {
        self.0.as_ffi()
    }
}

impl Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_u64())
    }
}

pub(crate) struct EntityMap {
    entities: SecondaryMap<EntityId, Box<dyn Any>>,
    pub accessed_entities: RefCell<FxHashSet<EntityId>>,
    ref_counts: Arc<RwLock<EntityRefCounts>>,
}

#[doc(hidden)]
pub(crate) struct EntityRefCounts {
    counts: SlotMap<EntityId, AtomicUsize>,
    dropped_entity_ids: Vec<EntityId>,
    #[cfg(any(test, feature = "leak-detection"))]
    leak_detector: LeakDetector,
}

impl EntityMap {
    pub fn new() -> Self {
        Self {
            entities: SecondaryMap::new(),
            accessed_entities: RefCell::new(FxHashSet::default()),
            ref_counts: Arc::new(RwLock::new(EntityRefCounts {
                counts: SlotMap::with_key(),
                dropped_entity_ids: Vec::new(),
                #[cfg(any(test, feature = "leak-detection"))]
                leak_detector: LeakDetector {
                    next_handle_id: 0,
                    entity_handles: HashMap::default(),
                },
            })),
        }
    }

    #[doc(hidden)]
    pub fn ref_counts_drop_handle(&self) -> Arc<RwLock<EntityRefCounts>> {
        self.ref_counts.clone()
    }

    /// Captures a snapshot of all entities that currently have alive handles.
    ///
    /// The returned [`LeakDetectorSnapshot`] can later be passed to
    /// [`assert_no_new_leaks`](Self::assert_no_new_leaks) to verify that no
    /// entities created after the snapshot are still alive.
    #[cfg(any(test, feature = "leak-detection"))]
    pub fn leak_detector_snapshot(&self) -> LeakDetectorSnapshot {
        self.ref_counts.read().leak_detector.snapshot()
    }

    /// Asserts that no entities created after `snapshot` still have alive handles.
    ///
    /// See [`LeakDetector::assert_no_new_leaks`] for details.
    #[cfg(any(test, feature = "leak-detection"))]
    pub fn assert_no_new_leaks(&self, snapshot: &LeakDetectorSnapshot) {
        self.ref_counts
            .read()
            .leak_detector
            .assert_no_new_leaks(snapshot)
    }

    /// Reserve a slot for an entity, which you can subsequently use with `insert`.
    pub fn reserve<T: 'static>(&self) -> Slot<T> {
        let id = self.ref_counts.write().counts.insert(1.into());
        Slot(Entity::new(id, Arc::downgrade(&self.ref_counts)))
    }

    /// Insert an entity into a slot obtained by calling `reserve`.
    pub fn insert<T>(&mut self, slot: Slot<T>, entity: T) -> Entity<T>
    where
        T: 'static,
    {
        let mut accessed_entities = self.accessed_entities.get_mut();
        accessed_entities.insert(slot.entity_id);

        let handle = slot.0;
        self.entities.insert(handle.entity_id, Box::new(entity));
        handle
    }

    /// 将实体移动到栈上。
    #[track_caller]
    pub fn lease<T>(&mut self, pointer: &Entity<T>) -> Lease<T> {
        self.assert_valid_context(pointer);
        let mut accessed_entities = self.accessed_entities.get_mut();
        accessed_entities.insert(pointer.entity_id);

        let entity = Some(
            self.entities
                .remove(pointer.entity_id)
                .unwrap_or_else(|| double_lease_panic::<T>("update")),
        );
        Lease {
            entity,
            id: pointer.entity_id,
            entity_type: PhantomData,
        }
    }

    /// 将实体移回后返回。
    pub fn end_lease<T>(&mut self, mut lease: Lease<T>) {
        self.entities.insert(lease.id, lease.entity.take().unwrap());
    }

    pub fn read<T: 'static>(&self, entity: &Entity<T>) -> &T {
        self.assert_valid_context(entity);
        let mut accessed_entities = self.accessed_entities.borrow_mut();
        accessed_entities.insert(entity.entity_id);

        self.entities
            .get(entity.entity_id)
            .and_then(|entity| entity.downcast_ref())
            .unwrap_or_else(|| double_lease_panic::<T>("read"))
    }

    /// 验证上下文是否正确
    fn assert_valid_context(&self, entity: &AnyEntity) {
        debug_assert!(
            Weak::ptr_eq(&entity.entity_map, &Arc::downgrade(&self.ref_counts)),
            "使用了错误上下文的实体"
        );
    }

    pub fn extend_accessed(&mut self, entities: &FxHashSet<EntityId>) {
        self.accessed_entities
            .get_mut()
            .extend(entities.iter().copied());
    }

    pub fn clear_accessed(&mut self) {
        self.accessed_entities.get_mut().clear();
    }

    pub fn take_dropped(&mut self) -> Vec<(EntityId, Box<dyn Any>)> {
        let mut ref_counts = &mut *self.ref_counts.write();
        let dropped_entity_ids = ref_counts.dropped_entity_ids.drain(..);
        let mut accessed_entities = self.accessed_entities.get_mut();

        dropped_entity_ids
            .filter_map(|entity_id| {
                let count = ref_counts.counts.remove(entity_id).unwrap();
                debug_assert_eq!(
                    count.load(SeqCst),
                    0,
                    "dropped an entity that was referenced"
                );
                accessed_entities.remove(&entity_id);
                // If the EntityId was allocated with `Context::reserve`,
                // the entity may not have been inserted.
                Some((entity_id, self.entities.remove(entity_id)?))
            })
            .collect()
    }
}

#[track_caller]
fn double_lease_panic<T>(operation: &str) -> ! {
    panic!(
        "无法在 {operation} {} 时操作，因为它正在被更新",
        std::any::type_name::<T>()
    )
}

pub(crate) struct Lease<T> {
    entity: Option<Box<dyn Any>>,
    pub id: EntityId,
    entity_type: PhantomData<T>,
}

impl<T: 'static> core::ops::Deref for Lease<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.entity.as_ref().unwrap().downcast_ref().unwrap()
    }
}

impl<T: 'static> core::ops::DerefMut for Lease<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.entity.as_mut().unwrap().downcast_mut().unwrap()
    }
}

impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        if self.entity.is_some() && !panicking() {
            panic!("Lease 必须通过 EntityMap::end_lease 结束")
        }
    }
}

#[derive(Deref, DerefMut)]
pub(crate) struct Slot<T>(Entity<T>);

/// A dynamically typed reference to a entity, which can be downcast into a `Entity<T>`.
pub struct AnyEntity {
    pub(crate) entity_id: EntityId,
    pub(crate) entity_type: TypeId,
    entity_map: Weak<RwLock<EntityRefCounts>>,
    #[cfg(any(test, feature = "leak-detection"))]
    handle_id: HandleId,
}

impl AnyEntity {
    fn new(
        id: EntityId,
        entity_type: TypeId,
        entity_map: Weak<RwLock<EntityRefCounts>>,
        #[cfg(any(test, feature = "leak-detection"))] type_name: &'static str,
    ) -> Self {
        Self {
            entity_id: id,
            entity_type,
            #[cfg(any(test, feature = "leak-detection"))]
            handle_id: entity_map
                .clone()
                .upgrade()
                .unwrap()
                .write()
                .leak_detector
                .handle_created(id, Some(type_name)),
            entity_map,
        }
    }

    /// Returns the id associated with this entity.
    #[inline]
    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    /// Returns the [TypeId] associated with this entity.
    #[inline]
    pub fn entity_type(&self) -> TypeId {
        self.entity_type
    }

    /// Converts this entity handle into a weak variant, which does not prevent it from being released.
    pub fn downgrade(&self) -> AnyWeakEntity {
        AnyWeakEntity {
            entity_id: self.entity_id,
            entity_type: self.entity_type,
            entity_ref_counts: self.entity_map.clone(),
        }
    }

    /// Converts this entity handle into a strongly-typed entity handle of the given type.
    /// If this entity handle is not of the specified type, returns itself as an error variant.
    pub fn downcast<T: 'static>(self) -> Result<Entity<T>, AnyEntity> {
        if TypeId::of::<T>() == self.entity_type {
            Ok(Entity {
                any_entity: self,
                entity_type: PhantomData,
            })
        } else {
            Err(self)
        }
    }
}

impl Clone for AnyEntity {
    fn clone(&self) -> Self {
        if let Some(entity_map) = self.entity_map.upgrade() {
            let entity_map = entity_map.read();
            let count = entity_map
                .counts
                .get(self.entity_id)
                .expect("检测到实体被过度释放");
            let prev_count = count.fetch_add(1, SeqCst);
            assert_ne!(prev_count, 0, "检测到实体被过度释放。");
        }

        Self {
            entity_id: self.entity_id,
            entity_type: self.entity_type,
            entity_map: self.entity_map.clone(),
            #[cfg(any(test, feature = "leak-detection"))]
            handle_id: self
                .entity_map
                .upgrade()
                .unwrap()
                .write()
                .leak_detector
                .handle_created(self.entity_id, None),
        }
    }
}

impl Drop for AnyEntity {
    fn drop(&mut self) {
        if let Some(entity_map) = self.entity_map.upgrade() {
            let entity_map = entity_map.upgradable_read();
            let count = entity_map
                .counts
                .get(self.entity_id)
                .expect("检测到句柄被过度释放。");
            let prev_count = count.fetch_sub(1, SeqCst);
            assert_ne!(prev_count, 0, "检测到实体被过度释放。");
            if prev_count == 1 {
                // 我们是此实体的最后一个引用，因此可以移除它。
                let mut entity_map = RwLockUpgradableReadGuard::upgrade(entity_map);
                entity_map.dropped_entity_ids.push(self.entity_id);
            }
        }

        #[cfg(any(test, feature = "leak-detection"))]
        if let Some(entity_map) = self.entity_map.upgrade() {
            entity_map
                .write()
                .leak_detector
                .handle_released(self.entity_id, self.handle_id)
        }
    }
}

impl<T> From<Entity<T>> for AnyEntity {
    #[inline]
    fn from(entity: Entity<T>) -> Self {
        entity.any_entity
    }
}

impl Hash for AnyEntity {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entity_id.hash(state);
    }
}

impl PartialEq for AnyEntity {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.entity_id == other.entity_id
    }
}

impl Eq for AnyEntity {}

impl Ord for AnyEntity {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.entity_id.cmp(&other.entity_id)
    }
}

impl PartialOrd for AnyEntity {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Debug for AnyEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyEntity")
            .field("entity_id", &self.entity_id.as_u64())
            .finish()
    }
}

/// A strong, well-typed reference to a struct which is managed
/// by GPUI
#[derive(Deref, DerefMut)]
pub struct Entity<T> {
    #[deref]
    #[deref_mut]
    pub(crate) any_entity: AnyEntity,
    pub(crate) entity_type: PhantomData<fn(T) -> T>,
}

impl<T> Sealed for Entity<T> {}

impl<T: 'static> Entity<T> {
    #[inline]
    fn new(id: EntityId, entity_map: Weak<RwLock<EntityRefCounts>>) -> Self
    where
        T: 'static,
    {
        Self {
            any_entity: AnyEntity::new(
                id,
                TypeId::of::<T>(),
                entity_map,
                #[cfg(any(test, feature = "leak-detection"))]
                std::any::type_name::<T>(),
            ),
            entity_type: PhantomData,
        }
    }

    /// Get the entity ID associated with this entity
    #[inline]
    pub fn entity_id(&self) -> EntityId {
        self.any_entity.entity_id
    }

    /// Downgrade this entity pointer to a non-retaining weak pointer
    #[inline]
    pub fn downgrade(&self) -> WeakEntity<T> {
        WeakEntity {
            any_entity: self.any_entity.downgrade(),
            entity_type: self.entity_type,
        }
    }

    /// Convert this into a dynamically typed entity.
    #[inline]
    pub fn into_any(self) -> AnyEntity {
        self.any_entity
    }

    /// Grab a reference to this entity from the context.
    #[inline]
    pub fn read<'a>(&self, cx: &'a App) -> &'a T {
        cx.entities.read(self)
    }

    /// Read the entity referenced by this handle with the given function.
    #[inline]
    pub fn read_with<R, C: AppContext>(&self, cx: &C, f: impl FnOnce(&T, &App) -> R) -> R {
        cx.read_entity(self, f)
    }

    /// Updates the entity referenced by this handle with the given function.
    #[inline]
    pub fn update<R, C: AppContext>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        cx.update_entity(self, update)
    }

    /// Updates the entity referenced by this handle with the given function.
    #[inline]
    pub fn as_mut<'a, C: AppContext>(&self, cx: &'a mut C) -> GpuiBorrow<'a, T> {
        cx.as_mut(self)
    }

    /// Updates the entity referenced by this handle with the given function.
    pub fn write<C: AppContext>(&self, cx: &mut C, value: T) {
        self.update(cx, |entity, cx| {
            *entity = value;
            cx.notify();
        })
    }

    /// Updates the entity referenced by this handle with the given function if
    /// the referenced entity still exists, within a visual context that has a window.
    /// Returns an error if the window has been closed.
    #[inline]
    pub fn update_in<R, C: VisualContext>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> C::Result<R> {
        cx.update_window_entity(self, update)
    }
}

impl<T> Clone for Entity<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            any_entity: self.any_entity.clone(),
            entity_type: self.entity_type,
        }
    }
}

impl<T> std::fmt::Debug for Entity<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entity")
            .field("entity_id", &self.any_entity.entity_id)
            .field("entity_type", &type_name::<T>())
            .finish()
    }
}

impl<T> Hash for Entity<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.any_entity.hash(state);
    }
}

impl<T> PartialEq for Entity<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.any_entity == other.any_entity
    }
}

impl<T> Eq for Entity<T> {}

impl<T> PartialEq<WeakEntity<T>> for Entity<T> {
    #[inline]
    fn eq(&self, other: &WeakEntity<T>) -> bool {
        self.any_entity.entity_id() == other.entity_id()
    }
}

impl<T: 'static> Ord for Entity<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.entity_id().cmp(&other.entity_id())
    }
}

impl<T: 'static> PartialOrd for Entity<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A type erased, weak reference to a entity.
#[derive(Clone)]
pub struct AnyWeakEntity {
    pub(crate) entity_id: EntityId,
    entity_type: TypeId,
    entity_ref_counts: Weak<RwLock<EntityRefCounts>>,
}

impl AnyWeakEntity {
    /// Get the entity ID associated with this weak reference.
    #[inline]
    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    /// Check if this weak handle can be upgraded, or if the entity has already been dropped
    pub fn is_upgradable(&self) -> bool {
        let ref_count = self
            .entity_ref_counts
            .upgrade()
            .and_then(|ref_counts| Some(ref_counts.read().counts.get(self.entity_id)?.load(SeqCst)))
            .unwrap_or(0);
        ref_count > 0
    }

    /// Upgrade this weak entity reference to a strong reference.
    pub fn upgrade(&self) -> Option<AnyEntity> {
        let ref_counts = &self.entity_ref_counts.upgrade()?;
        let ref_counts = ref_counts.read();
        let ref_count = ref_counts.counts.get(self.entity_id)?;

        if atomic_incr_if_not_zero(ref_count) == 0 {
            // entity_id is in dropped_entity_ids
            return None;
        }
        drop(ref_counts);

        Some(AnyEntity {
            entity_id: self.entity_id,
            entity_type: self.entity_type,
            entity_map: self.entity_ref_counts.clone(),
            #[cfg(any(test, feature = "leak-detection"))]
            handle_id: self
                .entity_ref_counts
                .upgrade()
                .unwrap()
                .write()
                .leak_detector
                .handle_created(self.entity_id, None),
        })
    }

    /// Asserts that the entity referenced by this weak handle has been fully released.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let entity = cx.new(|_| MyEntity::new());
    /// let weak = entity.downgrade();
    /// drop(entity);
    ///
    /// // Verify the entity was released
    /// weak.assert_released();
    /// ```
    ///
    /// # Debugging Leaks
    ///
    /// If this method panics due to leaked handles, set the `LEAK_BACKTRACE` environment
    /// variable to see where the leaked handles were allocated:
    ///
    /// ```bash
    /// LEAK_BACKTRACE=1 cargo test my_test
    /// ```
    ///
    /// # Panics
    ///
    /// - Panics if any strong handles to the entity are still alive.
    /// - Panics if the entity was recently dropped but cleanup hasn't completed yet
    ///   (resources are retained until the end of the effect cycle).
    #[cfg(any(test, feature = "leak-detection"))]
    pub fn assert_released(&self) {
        self.entity_ref_counts
            .upgrade()
            .unwrap()
            .write()
            .leak_detector
            .assert_released(self.entity_id);

        if self
            .entity_ref_counts
            .upgrade()
            .and_then(|ref_counts| Some(ref_counts.read().counts.get(self.entity_id)?.load(SeqCst)))
            .is_some()
        {
            panic!(
                "entity was recently dropped but resources are retained until the end of the effect cycle."
            )
        }
    }

    /// Creates a weak entity that can never be upgraded.
    pub fn new_invalid() -> Self {
        /// To hold the invariant that all ids are unique, and considering that slotmap
        /// increases their IDs from `0`, we can decrease ours from `u64::MAX` so these
        /// two will never conflict (u64 is way too large).
        static UNIQUE_NON_CONFLICTING_ID_GENERATOR: AtomicU64 = AtomicU64::new(u64::MAX);
        let entity_id = UNIQUE_NON_CONFLICTING_ID_GENERATOR.fetch_sub(1, SeqCst);

        Self {
            // Safety:
            //   Docs say this is safe but can be unspecified if slotmap changes the representation
            //   after `1.0.7`, that said, providing a valid entity_id here is not necessary as long
            //   as we guarantee that `entity_id` is never used if `entity_ref_counts` equals
            //   to `Weak::new()` (that is, it's unable to upgrade), that is the invariant that
            //   actually needs to be hold true.
            //
            //   And there is no sane reason to read an entity slot if `entity_ref_counts` can't be
            //   read in the first place, so we're good!
            entity_id: entity_id.into(),
            entity_type: TypeId::of::<()>(),
            entity_ref_counts: Weak::new(),
        }
    }
}

impl std::fmt::Debug for AnyWeakEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("entity_id", &self.entity_id)
            .field("entity_type", &self.entity_type)
            .finish()
    }
}

impl<T> From<WeakEntity<T>> for AnyWeakEntity {
    #[inline]
    fn from(entity: WeakEntity<T>) -> Self {
        entity.any_entity
    }
}

impl Hash for AnyWeakEntity {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entity_id.hash(state);
    }
}

impl PartialEq for AnyWeakEntity {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.entity_id == other.entity_id
    }
}

impl Eq for AnyWeakEntity {}

impl Ord for AnyWeakEntity {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.entity_id.cmp(&other.entity_id)
    }
}

impl PartialOrd for AnyWeakEntity {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A weak reference to a entity of the given type.
#[derive(Deref, DerefMut)]
pub struct WeakEntity<T> {
    #[deref]
    #[deref_mut]
    any_entity: AnyWeakEntity,
    entity_type: PhantomData<fn(T) -> T>,
}

impl<T> std::fmt::Debug for WeakEntity<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("entity_id", &self.any_entity.entity_id)
            .field("entity_type", &type_name::<T>())
            .finish()
    }
}

impl<T> Clone for WeakEntity<T> {
    fn clone(&self) -> Self {
        Self {
            any_entity: self.any_entity.clone(),
            entity_type: self.entity_type,
        }
    }
}

impl<T: 'static> WeakEntity<T> {
    /// Upgrade this weak entity reference into a strong entity reference
    pub fn upgrade(&self) -> Option<Entity<T>> {
        Some(Entity {
            any_entity: self.any_entity.upgrade()?,
            entity_type: self.entity_type,
        })
    }

    /// Updates the entity referenced by this handle with the given function if
    /// the referenced entity still exists. Returns an error if the entity has
    /// been released.
    pub fn update<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> Result<R>
    where
        C: AppContext,
    {
        let entity = self.upgrade().context("entity released")?;
        Ok(cx.update_entity(&entity, update))
    }

    /// Updates the entity referenced by this handle with the given function if
    /// the referenced entity still exists, within a visual context that has a window.
    /// Returns an error if the entity has been released.
    pub fn update_in<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> Result<R>
    where
        C: AppContext,
    {
        let entity = self.upgrade().context("entity released")?;
        cx.with_window(entity.entity_id(), |window, app| {
            entity.update(app, |entity, cx| update(entity, window, cx))
        })
        .context("entity has no current window")
    }

    /// Reads the entity referenced by this handle with the given function if
    /// the referenced entity still exists. Returns an error if the entity has
    /// been released.
    pub fn read_with<C, R>(&self, cx: &C, read: impl FnOnce(&T, &App) -> R) -> Result<R>
    where
        C: AppContext,
    {
        let entity = self.upgrade().context("entity released")?;
        Ok(cx.read_entity(&entity, read))
    }

    /// Create a new weak entity that can never be upgraded.
    #[inline]
    pub fn new_invalid() -> Self {
        Self {
            any_entity: AnyWeakEntity::new_invalid(),
            entity_type: PhantomData,
        }
    }
}

impl<T> Hash for WeakEntity<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.any_entity.hash(state);
    }
}

impl<T> PartialEq for WeakEntity<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.any_entity == other.any_entity
    }
}

impl<T> Eq for WeakEntity<T> {}

impl<T> PartialEq<Entity<T>> for WeakEntity<T> {
    #[inline]
    fn eq(&self, other: &Entity<T>) -> bool {
        self.entity_id() == other.any_entity.entity_id()
    }
}

impl<T: 'static> Ord for WeakEntity<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.entity_id().cmp(&other.entity_id())
    }
}

impl<T: 'static> PartialOrd for WeakEntity<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 控制是否在创建实体句柄时捕获回溯。
///
/// 将 `LEAK_BACKTRACE` 环境变量设置为任何非空值以启用回溯捕获。
/// 这有助于识别泄漏的句柄是在哪里分配的。
#[cfg(any(test, feature = "leak-detection"))]
static LEAK_BACKTRACE: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var("LEAK_BACKTRACE").is_ok_and(|b| !b.is_empty()));

/// 特定实体句柄实例的唯一标识符。
///
/// 这与 `EntityId` 不同——虽然多个句柄可以指向同一个实体（相同的 `EntityId`），
/// 但每个句柄都有自己唯一的 `HandleId`。
#[cfg(any(test, feature = "leak-detection"))]
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub(crate) struct HandleId {
    id: u64,
}

/// 跟踪实体句柄分配以检测泄漏。
///
/// 泄漏检测器在测试中以及 `leak-detection` 特性激活时启用。
/// 它跟踪创建和释放的每个 `Entity<T>` 和 `AnyEntity` 句柄，
/// 允许你验证指向实体的所有句柄是否都已正确丢弃。
///
/// # 泄漏是如何发生的？
///
/// 实体是可以拥有其他实体的引用计数结构，
/// 从而形成循环。如果创建了这样的强引用计数循环，
/// 此循环中所有参与的强实体将有效地泄漏，因为它们无法再被释放。
///
/// 如果实体拥有它自身拥有强引用的任务或订阅，也可能发生循环。
///
/// # 用法
///
/// 你可以使用 `WeakEntity::assert_released` 或 `AnyWeakEntity::assert_released`
/// 验证实体是否已完全释放：
///
/// ```ignore
/// let entity = cx.new(|_| MyEntity::new());
/// let weak = entity.downgrade();
/// drop(entity);
///
/// // 如果实体的任何句柄仍然存活，这将 panic
/// weak.assert_released();
/// ```
///
/// # 调试泄漏
///
/// 检测到泄漏时，检测器将 panic 并显示有关泄漏句柄的信息。
/// 要查看泄漏句柄是在哪里分配的，请设置 `LEAK_BACKTRACE` 环境变量：
///
/// ```bash
/// LEAK_BACKTRACE=1 cargo test my_test
/// ```
///
/// 这将捕获并显示每个泄漏句柄的回溯，帮助你识别泄漏句柄的创建位置。
///
/// # 工作原理
///
/// - 当创建实体句柄时（通过 `Entity::new`、`Entity::clone` 或 `WeakEntity::upgrade`），
///   调用 `handle_created` 注册句柄。
/// - 当句柄被丢弃时，`handle_released` 将其从跟踪中移除。
/// - `assert_released` 验证实体没有剩余句柄。
#[cfg(any(test, feature = "leak-detection"))]
pub(crate) struct LeakDetector {
    next_handle_id: u64,
    entity_handles: HashMap<EntityId, EntityLeakData>,
}

/// 特定时间点存活实体集合的快照。
///
/// 由 [`LeakDetector::snapshot`] 创建。之后可以传递给
/// [`LeakDetector::assert_no_new_leaks`] 以验证在快照和当前状态之间
/// 没有新的实体句柄残留。
#[cfg(any(test, feature = "leak-detection"))]
pub struct LeakDetectorSnapshot {
    entity_ids: collections::HashSet<EntityId>,
}

#[cfg(any(test, feature = "leak-detection"))]
struct EntityLeakData {
    handles: HashMap<HandleId, Option<backtrace::Backtrace>>,
    type_name: &'static str,
}

#[cfg(any(test, feature = "leak-detection"))]
impl LeakDetector {
    /// 记录已为给定实体创建了新句柄。
    ///
    /// 返回唯一的 `HandleId`，必须在句柄丢弃时传递给 `handle_released`。
    /// 如果设置了 `LEAK_BACKTRACE`，则在分配点捕获回溯。
    #[track_caller]
    pub fn handle_created(
        &mut self,
        entity_id: EntityId,
        type_name: Option<&'static str>,
    ) -> HandleId {
        let id = crate::post_inc(&mut self.next_handle_id);
        let handle_id = HandleId { id };
        let handles = self
            .entity_handles
            .entry(entity_id)
            .or_insert_with(|| EntityLeakData {
                handles: HashMap::default(),
                type_name: type_name.unwrap_or("<unknown>"),
            });
        handles.handles.insert(
            handle_id,
            LEAK_BACKTRACE.then(backtrace::Backtrace::new_unresolved),
        );
        handle_id
    }

    /// 记录句柄已被释放（丢弃）。
    ///
    /// 这将从跟踪中移除句柄。`handle_id` 应与分配句柄时
    /// `handle_created` 返回的相同。
    pub fn handle_released(&mut self, entity_id: EntityId, handle_id: HandleId) {
        if let std::collections::hash_map::Entry::Occupied(mut data) =
            self.entity_handles.entry(entity_id)
        {
            data.get_mut().handles.remove(&handle_id);
            if data.get().handles.is_empty() {
                data.remove();
            }
        }
    }

    /// 断言给定实体的所有句柄都已释放。
    ///
    /// # Panics
    ///
    /// 如果实体的任何句柄仍然存活则 panic。panic 消息
    /// 包含每个泄漏句柄的回溯（如果设置了 `LEAK_BACKTRACE`），
    /// 否则建议设置环境变量以获取更多信息。
    pub fn assert_released(&mut self, entity_id: EntityId) {
        use std::fmt::Write as _;

        if let Some(data) = self.entity_handles.remove(&entity_id) {
            let mut out = String::new();
            for (_, backtrace) in data.handles {
                if let Some(mut backtrace) = backtrace {
                    backtrace.resolve();
                    let backtrace = BacktraceFormatter(backtrace);
                    writeln!(out, "Leaked handle:\n{:?}", backtrace).unwrap();
                } else {
                    writeln!(out, "泄漏的句柄：（导出 LEAK_BACKTRACE 以查找分配位置）").unwrap();
                }
            }
            panic!("{} 的句柄泄漏:\n{out}", data.type_name);
        }
    }

    /// 捕获当前所有具有存活句柄的实体 ID 的快照。
    ///
    /// 返回的 [`LeakDetectorSnapshot`] 之后可以传递给
    /// [`assert_no_new_leaks`](Self::assert_no_new_leaks) 以验证快照之后创建的
    /// 实体没有仍然存活的。
    pub fn snapshot(&self) -> LeakDetectorSnapshot {
        LeakDetectorSnapshot {
            entity_ids: self.entity_handles.keys().copied().collect(),
        }
    }

    /// 断言快照之后创建的实体没有仍然具有存活句柄的。
    ///
    /// 快照时已跟踪的实体将被忽略，
    /// 即使它们仍然有句柄。只有*新*实体（那些
    /// `EntityId` 不在快照中的）才被视为泄漏。
    ///
    /// # Panics
    ///
    /// 如果存在任何新实体句柄则 panic。panic 消息列出每个
    /// 泄漏实体及其类型名称，并在设置 `LEAK_BACKTRACE` 时包含分配位置回溯。
    pub fn assert_no_new_leaks(&self, snapshot: &LeakDetectorSnapshot) {
        use std::fmt::Write as _;

        let mut out = String::new();
        for (entity_id, data) in &self.entity_handles {
            if snapshot.entity_ids.contains(entity_id) {
                continue;
            }
            for (_, backtrace) in &data.handles {
                if let Some(backtrace) = backtrace {
                    let mut backtrace = backtrace.clone();
                    backtrace.resolve();
                    let backtrace = BacktraceFormatter(backtrace);
                    writeln!(
                        out,
                        "实体 {} ({entity_id:?}) 的泄漏句柄:\n{:?}",
                        data.type_name, backtrace
                    )
                    .unwrap();
                } else {
                    writeln!(
                        out,
                        "实体 {} ({entity_id:?}) 的泄漏句柄：（导出 LEAK_BACKTRACE 以查找分配位置）",
                        data.type_name
                    )
                    .unwrap();
                }
            }
        }

        if !out.is_empty() {
            panic!("自快照以来检测到新实体泄漏:\n{out}");
        }
    }
}

#[cfg(any(test, feature = "leak-detection"))]
impl Drop for LeakDetector {
    fn drop(&mut self) {
        use std::fmt::Write;

        if self.entity_handles.is_empty() || std::thread::panicking() {
            return;
        }

        let mut out = String::new();
        for (entity_id, data) in self.entity_handles.drain() {
            for (_handle, backtrace) in data.handles {
                if let Some(mut backtrace) = backtrace {
                    backtrace.resolve();
                    let backtrace = BacktraceFormatter(backtrace);
                    writeln!(
                        out,
                        "实体 {} ({entity_id:?}) 的泄漏句柄:\n{:?}",
                        data.type_name, backtrace
                    )
                    .unwrap();
                } else {
                    writeln!(
                        out,
                        "实体 {} ({entity_id:?}) 的泄漏句柄：（导出 LEAK_BACKTRACE 以查找分配位置）",
                        data.type_name
                    )
                    .unwrap();
                }
            }
        }
        panic!("退出时存在泄漏的句柄:\n{out}");
    }
}

#[cfg(any(test, feature = "leak-detection"))]
struct BacktraceFormatter(backtrace::Backtrace);

#[cfg(any(test, feature = "leak-detection"))]
impl fmt::Debug for BacktraceFormatter {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        use backtrace::{BacktraceFmt, BytesOrWideString, PrintFmt};

        let style = if fmt.alternate() {
            PrintFmt::Full
        } else {
            PrintFmt::Short
        };

        // 打印路径时，如果存在则尝试剥离 cwd，否则
        // 我们只按原样打印路径。注意我们也只对
        // 短格式这样做，因为如果是完整格式，我们可能想打印所有内容。
        let cwd = std::env::current_dir();
        let mut print_path = move |fmt: &mut fmt::Formatter<'_>, path: BytesOrWideString<'_>| {
            let path = path.into_path_buf();
            if style != PrintFmt::Full {
                if let Ok(cwd) = &cwd {
                    if let Ok(suffix) = path.strip_prefix(cwd) {
                        return fmt::Display::fmt(&suffix.display(), fmt);
                    }
                }
            }
            fmt::Display::fmt(&path.display(), fmt)
        };

        let mut f = BacktraceFmt::new(fmt, style, &mut print_path);
        f.add_context()?;
        let mut strip = true;
        for frame in self.0.frames() {
            if let [symbol, ..] = frame.symbols()
                && let Some(name) = symbol.name()
                && let Some(filename) = name.as_str()
            {
                match filename {
                    "test::run_test_in_process"
                    | "scheduler::executor::spawn_local_with_source_location::impl$1::poll<core::pin::Pin<alloc::boxed::Box<dyn$<core::future::future::Future<assoc$<Output,enum2$<core::result::Result<workspace::OpenResult,anyhow::Error> > > > >,alloc::alloc::Global> > >" => {
                        strip = true
                    }
                    "rgpui::app::entity_map::LeakDetector::handle_created" => {
                        strip = false;
                        continue;
                    }
                    "zed::main" => {
                        strip = true;
                        f.frame().backtrace_frame(frame)?;
                    }
                    _ => {}
                }
            }
            if strip {
                continue;
            }
            f.frame().backtrace_frame(frame)?;
        }
        f.finish()?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::EntityMap;

    struct TestEntity {
        pub i: i32,
    }

    #[test]
    fn test_entity_map_slot_assignment_before_cleanup() {
        // 测试在 take_dropped 之前槽位不会被重用。
        let mut entity_map = EntityMap::new();

        let slot = entity_map.reserve::<TestEntity>();
        entity_map.insert(slot, TestEntity { i: 1 });

        let slot = entity_map.reserve::<TestEntity>();
        entity_map.insert(slot, TestEntity { i: 2 });

        let dropped = entity_map.take_dropped();
        assert_eq!(dropped.len(), 2);

        assert_eq!(
            dropped
                .into_iter()
                .map(|(_, entity)| entity.downcast::<TestEntity>().unwrap().i)
                .collect::<Vec<i32>>(),
            vec![1, 2],
        );
    }

    #[test]
    fn test_entity_map_weak_upgrade_before_cleanup() {
        // 测试在 take_dropped 之前弱句柄不会被升级
        let mut entity_map = EntityMap::new();

        let slot = entity_map.reserve::<TestEntity>();
        let handle = entity_map.insert(slot, TestEntity { i: 1 });
        let weak = handle.downgrade();
        drop(handle);

        let strong = weak.upgrade();
        assert_eq!(strong, None);

        let dropped = entity_map.take_dropped();
        assert_eq!(dropped.len(), 1);

        assert_eq!(
            dropped
                .into_iter()
                .map(|(_, entity)| entity.downcast::<TestEntity>().unwrap().i)
                .collect::<Vec<i32>>(),
            vec![1],
        );
    }

    #[test]
    fn test_leak_detector_snapshot_no_leaks() {
        let mut entity_map = EntityMap::new();

        let slot = entity_map.reserve::<TestEntity>();
        let pre_existing = entity_map.insert(slot, TestEntity { i: 1 });

        let snapshot = entity_map.leak_detector_snapshot();

        let slot = entity_map.reserve::<TestEntity>();
        let temporary = entity_map.insert(slot, TestEntity { i: 2 });
        drop(temporary);

        entity_map.assert_no_new_leaks(&snapshot);

        drop(pre_existing);
    }

    #[test]
    #[should_panic(expected = "New entity leaks detected since snapshot")]
    fn test_leak_detector_snapshot_detects_new_leak() {
        let mut entity_map = EntityMap::new();

        let slot = entity_map.reserve::<TestEntity>();
        let pre_existing = entity_map.insert(slot, TestEntity { i: 1 });

        let snapshot = entity_map.leak_detector_snapshot();

        let slot = entity_map.reserve::<TestEntity>();
        let leaked = entity_map.insert(slot, TestEntity { i: 2 });

        // `leaked` 仍然存活，所以这应该 panic。
        entity_map.assert_no_new_leaks(&snapshot);

        drop(pre_existing);
        drop(leaked);
    }
}
