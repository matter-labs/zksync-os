use crate::system::errors::{InternalError, SystemError};

use super::{
    history_map::{
        CacheSnapshotId, HistoryMap, HistoryMapItemRef, HistoryMapItemRefMut, TransactionId,
    },
    WarmStorageKey, WarmStorageValue,
};
use core::{alloc::Allocator, fmt::Debug, ops::Bound};

// TODO move to some proper place

#[derive(Default, Copy, Clone, Eq, PartialEq, Debug)]
pub enum Appearance {
    #[default]
    Unset,
    Retrieved,
    Updated,
    Deconstructed,
}

#[derive(Clone, Default)]
/// A cache entry. User facing struct.
pub struct CacheRecord<V, M> {
    appearance: Appearance,
    value: V,
    metadata: M,
}

impl<V, M: Default> CacheRecord<V, M> {
    pub fn new(value: V, appearance: Appearance) -> Self {
        Self {
            appearance,
            value,
            metadata: Default::default(),
        }
    }
}

impl<V, M> CacheRecord<V, M> {
    pub fn appearance(&self) -> Appearance {
        self.appearance
    }

    pub fn value(&self) -> &V {
        &self.value
    }

    pub fn metadata(&self) -> &M {
        &self.metadata
    }

    #[must_use]
    pub fn update<F>(&mut self, f: F) -> Result<(), InternalError>
    where
        F: FnOnce(&mut V, &mut M) -> Result<(), InternalError>,
    {
        if self.appearance != Appearance::Deconstructed {
            self.appearance = Appearance::Updated
        };

        f(&mut self.value, &mut self.metadata)
    }

    #[must_use]
    /// Updates the metadata and retains the appearance.
    pub fn update_metadata<F>(&mut self, f: F) -> Result<(), SystemError>
    where
        F: FnOnce(&mut M) -> Result<(), SystemError>,
    {
        f(&mut self.metadata)
    }

    /// Sets appearance to deconstructed. The value itself remains untouched.
    pub fn deconstruct(&mut self) {
        self.appearance = Appearance::Deconstructed;
    }

    /// Sets appearance to unset. The value itself remains untouched.
    pub fn unset(&mut self) {
        self.appearance = Appearance::Unset;
    }
}

pub struct IoCacheItemRef<'a, K: Clone, V, M, A: Allocator + Clone>(
    HistoryMapItemRef<'a, K, CacheRecord<V, M>, A>,
);

impl<'a, K, V, M, A> IoCacheItemRef<'a, K, V, M, A>
where
    K: Clone,
    A: Allocator + Clone,
{
    pub fn key(&self) -> &'a K {
        &self.0.key()
    }

    pub fn current(&self) -> &CacheRecord<V, M> {
        &self.0.current()
    }

    pub fn last(&self) -> &CacheRecord<V, M> {
        &self.0.last()
    }

    // TODO remove?
    pub fn diff_operands_total(&self) -> Option<(&CacheRecord<V, M>, &CacheRecord<V, M>)> {
        self.0.diff_operands_total()
    }
}

pub struct IoCacheItemRefMut<'a, K: Clone, V, M, A: Allocator + Clone>(
    HistoryMapItemRefMut<'a, K, CacheRecord<V, M>, A>,
);

impl<'a, K, V, M, A> IoCacheItemRefMut<'a, K, V, M, A>
where
    K: Clone + Debug,
    M: Clone,
    V: Clone,
    A: Allocator + Clone,
{
    pub fn current(&self) -> &CacheRecord<V, M> {
        self.0.current()
    }

    // TODO remove?
    pub fn diff_operands_tx(&self) -> Option<(&CacheRecord<V, M>, &CacheRecord<V, M>)> {
        self.0.diff_operands_tx()
    }

    // TODO remove?
    #[allow(dead_code)]
    pub fn diff_operands_total(&self) -> Option<(&CacheRecord<V, M>, &CacheRecord<V, M>)> {
        self.0.diff_operands_total()
    }

    #[must_use]
    /// Updates the metadata and retains the appearance.
    /// TODO: appearance kinda part of metadata actually
    pub fn update_metadata<F>(&mut self, f: F) -> Result<(), SystemError>
    where
        F: FnOnce(&mut M) -> Result<(), SystemError>,
    {
        self.0.update(|v| v.update_metadata(f))
    }

    #[must_use]
    pub fn update<F>(&mut self, f: F) -> Result<(), InternalError>
    where
        F: FnOnce(&mut V, &mut M) -> Result<(), InternalError>,
    {
        self.0.update(|v| v.update(f))
    }
}

impl<'a, K, V, M, A> IoCacheItemRefMut<'a, K, V, M, A>
where
    K: Clone + Debug,
    V: Clone,
    M: Clone,
    A: Allocator + Clone,
{
    #[must_use]
    /// Sets appearance to deconstructed. The value itself remains untouched.
    pub fn deconstruct(&mut self) -> Result<(), InternalError> {
        self.0.update(|v| {
            v.deconstruct();
            Ok(())
        })
    }

    /// Sets appearance to unset. The value itself remains untouched.
    pub fn unset(&mut self) -> Result<(), InternalError> {
        self.0.update(|v| {
            v.unset();
            Ok(())
        })
    }
}

pub struct IoCache<K, V, M, A: Allocator + Clone> {
    history_map: HistoryMap<K, CacheRecord<V, M>, A>,
}

impl<K, V, M: Default, A> IoCache<K, V, M, A>
where
    K: Ord + Clone + Debug,
    A: Allocator + Clone,
{
    pub fn new(alloc: A) -> Self {
        Self {
            history_map: HistoryMap::new(alloc),
        }
    }

    pub fn get<'s>(&'s mut self, key: &'s K) -> Option<IoCacheItemRef<'s, K, V, M, A>> {
        self.history_map.get(key).map(|x| IoCacheItemRef(x))
    }

    pub fn get_mut<'s>(&'s mut self, key: &'s K) -> Option<IoCacheItemRefMut<'s, K, V, M, A>> {
        self.history_map.get_mut(key).map(|x| IoCacheItemRefMut(x))
    }

    pub fn get_or_insert<'s, E>(
        &'s mut self,
        key: &'s K,
        spawn_v: impl FnOnce() -> Result<CacheRecord<V, M>, E>,
    ) -> Result<IoCacheItemRefMut<'s, K, V, M, A>, E> {
        Ok(IoCacheItemRefMut(
            self.history_map.get_or_insert(key, spawn_v)?,
        ))
    }

    pub fn snapshot(&mut self, tx_id: TransactionId) -> CacheSnapshotId {
        self.history_map.snapshot(tx_id)
    }

    /// Rollbacks the data to the state before the provided `snapshot_id`.
    pub fn rollback(&mut self, snapshot_id: CacheSnapshotId) {
        self.history_map.rollback(snapshot_id);
    }

    /// Commits changes up to this point and frees memory taken by snapshots that can't be
    /// rollbacked to.
    /// TODO rename to reset or smth
    pub fn commit(&mut self) {
        self.history_map.commit();
    }

    // TODO check usage
    pub fn for_total_diff_operands<F, E>(&self, do_fn: F) -> Result<(), E>
    where
        F: FnMut(&CacheRecord<V, M>, &CacheRecord<V, M>, &K) -> Result<(), E>,
    {
        self.history_map.for_total_diff_operands(do_fn)
    }

    // TODO used to cleanup in storage and transient storage
    pub fn for_each_range<F>(
        &mut self,
        range: (Bound<&K>, Bound<&K>),
        mut do_fn: F,
    ) -> Result<(), InternalError>
    where
        F: FnMut(IoCacheItemRefMut<K, V, M, A>) -> Result<(), InternalError>,
    {
        self.history_map
            .for_each_range(range, |x| do_fn(IoCacheItemRefMut(x)))
    }

    // TODO only for new preimages publication storage
    pub fn iter(&self) -> impl Iterator<Item = IoCacheItemRef<'_, K, V, M, A>> + Clone {
        self.history_map.iter().map(|x| IoCacheItemRef(x))
    }

    // TODO use only for account cache
    pub fn iter_altered_since_commit(
        &self,
    ) -> impl Iterator<Item = IoCacheItemRef<'_, K, V, M, A>> {
        self.history_map
            .iter_altered_since_commit()
            .map(|x| IoCacheItemRef(x))
    }
}

impl<A, M: Default> IoCache<WarmStorageKey, crate::utils::Bytes32, M, A>
where
    A: Allocator + Clone,
{
    pub fn iter_as_storage_types(
        &self,
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone + use<'_, A, M> {
        self.iter().map(|item| {
            let current_value = item.current();
            let initial_value = item.last();
            (
                *item.0.key(),
                // Using the WarmStorageValue temporarily till it's outed from the codebase. We're
                // not actually 'using' it.
                WarmStorageValue {
                    current_value: current_value.value,
                    is_new_storage_slot: initial_value.appearance == Appearance::Unset,
                    initial_value: initial_value.value,
                    initial_value_used: true,
                    ..Default::default()
                },
            )
        })
    }
}
