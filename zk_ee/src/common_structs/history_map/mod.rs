//! Contains a key-value map that allows reverting items state.

mod element_pool;
pub mod element_with_history;

use crate::common_structs::history_map::element_with_history::HistoryRecord;
use crate::internal_error;
use crate::{system::errors::InternalError, utils::stack_linked_list::StackLinkedList};
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use core::{alloc::Allocator, fmt::Debug, ops::Bound};
use element_pool::ElementPool;
use element_with_history::ElementWithHistory;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct CacheSnapshotId(usize);

impl CacheSnapshotId {
    pub fn new() -> Self {
        Self(0)
    }
    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

/// A key-value map with history. State can be reverted to snapshots.
/// The snapshots are created using `Self::snapshot(...)` method.
///
/// Structure:
/// [ keys ] => [ history ] := [ snapshot 0 .. snapshot n ].
pub struct HistoryMap<K, V, A: Allocator + Clone> {
    /// Map from key to history of an element
    btree: BTreeMap<K, ElementWithHistory<V, A>, A>,
    state: HistoryMapState<K, A>,
    /// Manages memory allocations for history records, reuses old allocations for optimization
    records_memory_pool: ElementPool<V, A>,
}

struct HistoryMapState<K, A: Allocator + Clone> {
    next_snapshot_id: CacheSnapshotId,
    /// State can't be rolled back further than frozen snapshot id. Useful for transactions boundaries
    frozen_snapshot_id: CacheSnapshotId,
    /// List of updated elements that were not yet "frozen"
    pending_updated_elements: StackLinkedList<(K, CacheSnapshotId), A>,
    alloc: A,
}

impl<K, V, A> HistoryMap<K, V, A>
where
    K: Ord + Clone + Debug,
    A: Allocator + Clone,
{
    pub fn new(alloc: A) -> Self {
        Self {
            btree: BTreeMap::new_in(alloc.clone()),
            state: HistoryMapState {
                alloc: alloc.clone(),
                // Initial values will be associated with snapshot 0 (so they can't be reverted)
                next_snapshot_id: CacheSnapshotId(1),
                frozen_snapshot_id: CacheSnapshotId(0),
                pending_updated_elements: StackLinkedList::empty(alloc.clone()),
            },
            records_memory_pool: ElementPool::new(alloc),
        }
    }

    /// Get history of an element by key
    pub fn get<'s>(&'s mut self, key: &'s K) -> Option<HistoryMapItemRef<'s, K, V, A>> {
        self.btree
            .get(key)
            .map(|ec| HistoryMapItemRef { key, history: ec })
    }

    /// Get history of an element by key, mutable
    pub fn get_mut<'s>(&'s mut self, key: &'s K) -> Option<HistoryMapItemRefMut<'s, K, V, A>> {
        self.btree.get_mut(key).map(|ec| HistoryMapItemRefMut {
            key,
            history: ec,
            cache_state: &mut self.state,
            records_memory_pool: &mut self.records_memory_pool,
        })
    }

    /// Get history of an element by key or use callback to insert initial value
    pub fn get_or_insert<'s, E>(
        &'s mut self,
        key: &'s K,
        spawn_v: impl FnOnce() -> Result<V, E>,
    ) -> Result<HistoryMapItemRefMut<'s, K, V, A>, E> {
        let entry = self.btree.entry(key.clone());

        let v = match entry {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(vacant_entry) => {
                let v = spawn_v()?;
                vacant_entry.insert(ElementWithHistory::new(
                    v,
                    &mut self.records_memory_pool,
                    self.state.alloc.clone(),
                ))
            }
        };

        Ok(HistoryMapItemRefMut {
            key,
            history: v,
            cache_state: &mut self.state,
            records_memory_pool: &mut self.records_memory_pool,
        })
    }

    /// Save current state as a snapshot. Returns corresponding snapshot id
    pub fn snapshot(&mut self) -> CacheSnapshotId {
        let snapshot_id = self.state.next_snapshot_id;
        self.state.next_snapshot_id.increment();
        snapshot_id
    }

    #[must_use]
    /// Rollbacks the data to the state at the provided `snapshot_id`.
    pub fn rollback(&mut self, snapshot_id: CacheSnapshotId) -> Result<(), InternalError> {
        if snapshot_id < self.state.frozen_snapshot_id {
            return Err(internal_error!(
                "History map: rollback below frozen snapshot"
            ));
        }

        if snapshot_id >= self.state.next_snapshot_id {
            return Err(internal_error!(
                "History map: rollback to non-existent snapshot"
            ));
        }

        // Go over all elements changed since last `commit` and roll them back
        let mut node = self.state.pending_updated_elements.pop();
        loop {
            match node {
                None => break,
                Some((key, update_snapshot_id)) => {
                    // The items in the address_snapshot_updates are ordered chronologically.
                    if update_snapshot_id <= snapshot_id {
                        self.state
                            .pending_updated_elements
                            .push((key, update_snapshot_id));
                        break;
                    }

                    let item = self
                        .btree
                        .get_mut(&key)
                        .expect("We've updated this, so it must be present.");

                    item.rollback(&mut self.records_memory_pool, snapshot_id);

                    node = self.state.pending_updated_elements.pop();
                }
            }
        }

        Ok(())
    }

    /// Commits (freezes) changes up to this point and frees memory taken by snapshots that can't be
    /// rollbacked to.
    pub fn commit(&mut self) {
        self.state.frozen_snapshot_id = self.snapshot();

        // Go over all elements changed since last `commit` and `commit` their history
        for (key, _) in self.state.pending_updated_elements.iter() {
            let item = self
                .btree
                .get_mut(key)
                .expect("We've updated this, so it must be present.");

            item.commit(&mut self.records_memory_pool);
        }

        // We've committed, so we don't need those changes anymore.
        self.state.pending_updated_elements = StackLinkedList::empty(self.state.alloc.clone());
    }

    /// Applies callback `do_fn` to all pairs (initial_value, current_value) that have more than 1 (initial) record
    pub fn apply_to_all_updated_elements<F, E>(&self, mut do_fn: F) -> Result<(), E>
    where
        F: FnMut(&V, &V, &K) -> Result<(), E>,
    {
        for (k, v) in &self.btree {
            if let Some((initial, last)) = v.get_initial_and_last_values() {
                do_fn(initial, last, k)?;
            }
        }

        Ok(())
    }

    /// Applies callback `do_fn` to elements in range
    pub fn for_each_range<F>(
        &mut self,
        range: (Bound<&K>, Bound<&K>),
        mut do_fn: F,
    ) -> Result<(), InternalError>
    where
        F: FnMut(HistoryMapItemRefMut<K, V, A>) -> Result<(), InternalError>,
    {
        for (k, v) in self.btree.range_mut(range) {
            do_fn(HistoryMapItemRefMut {
                key: &k,
                history: v,
                cache_state: &mut self.state,
                records_memory_pool: &mut self.records_memory_pool,
            })?
        }

        Ok(())
    }

    /// Iterate over all elements in map
    pub fn iter(&self) -> impl Iterator<Item = HistoryMapItemRef<'_, K, V, A>> + Clone {
        self.btree
            .iter()
            .map(|(k, v)| HistoryMapItemRef { key: k, history: v })
    }

    /// Iterate over all elements that changed since last commit
    pub fn iter_altered_since_commit(
        &self,
    ) -> impl Iterator<Item = HistoryMapItemRef<'_, K, V, A>> {
        self.state
            .pending_updated_elements
            .iter()
            .map(|(k, _)| HistoryMapItemRef {
                key: k,
                history: self
                    .btree
                    .get(k)
                    .expect("We've updated this, so it must be present."),
            })
    }

    /// Iterate over the head of each element altered since last commit
    pub fn apply_to_last_record_of_pending_changes<F>(
        &mut self,
        mut do_fn: F,
    ) -> Result<(), InternalError>
    where
        F: FnMut(&K, &mut HistoryRecord<V>) -> Result<(), InternalError>,
    {
        for (k, _v) in self.state.pending_updated_elements.iter() {
            do_fn(k, unsafe { self.btree.get_mut(&k).unwrap().head.as_mut() })?
        }

        Ok(())
    }
}

/// External reference to element's history
pub struct HistoryMapItemRef<'a, K: Clone, V, A: Allocator + Clone> {
    key: &'a K,
    history: &'a ElementWithHistory<V, A>,
}

impl<'a, K, V, A> HistoryMapItemRef<'a, K, V, A>
where
    K: Clone,
    A: Allocator + Clone,
{
    pub fn key(&self) -> &'a K {
        &self.key
    }

    pub fn current(&self) -> &V {
        unsafe { &self.history.head.as_ref().value }
    }

    pub fn initial(&self) -> &V {
        unsafe { &self.history.initial.as_ref().value }
    }

    /// Returns (initial_value, current_value) if any
    pub fn get_initial_and_last_values(&self) -> Option<(&V, &V)> {
        self.history.get_initial_and_last_values()
    }
}

/// External mutable reference to element's history
pub struct HistoryMapItemRefMut<'a, K: Clone, V, A: Allocator + Clone> {
    history: &'a mut ElementWithHistory<V, A>,
    cache_state: &'a mut HistoryMapState<K, A>,
    records_memory_pool: &'a mut ElementPool<V, A>,
    key: &'a K,
}

impl<'a, K, V, A> HistoryMapItemRefMut<'a, K, V, A>
where
    K: Clone + Debug,
    V: Clone,
    A: Allocator + Clone,
{
    pub fn current(&self) -> &V {
        unsafe { &self.history.head.as_ref().value }
    }

    pub fn initial(&self) -> &V {
        unsafe { &self.history.initial.as_ref().value }
    }

    #[allow(dead_code)]
    /// Returns (initial_value, current_value) if any
    pub fn get_initial_and_last_values(&self) -> Option<(&V, &V)> {
        self.history.get_initial_and_last_values()
    }

    #[must_use]
    /// Use callback `f` to add new record and update element
    pub fn update<F, E>(&mut self, f: F) -> Result<(), E>
    where
        F: FnOnce(&mut V) -> Result<(), E>,
    {
        let last_history_record = unsafe { self.history.head.as_mut() };

        if last_history_record.touch_ss_id == self.cache_state.next_snapshot_id {
            // We're in the context of the current snapshot: there are changes that we will simply override
            f(&mut last_history_record.value)
        } else {
            // The item was last updated before the current snapshot.

            let mut new = self.records_memory_pool.create_element(
                last_history_record.value.clone(),
                Some(self.history.head),
                self.cache_state.next_snapshot_id,
            );

            unsafe {
                f(&mut new.as_mut().value)?;
            }

            self.history.add_new_record(new);

            self.cache_state
                .pending_updated_elements
                .push((self.key.clone(), self.cache_state.next_snapshot_id));

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::Global;

    use super::HistoryMap;

    #[test]
    fn miri_retrieve_single_elem() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        let v = map.get_or_insert::<()>(&1, || Ok(1)).unwrap();

        assert_eq!(1, *v.current());
    }

    #[test]
    fn miri_diff_elem_total() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok(1)).unwrap();

        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        let (l, r) = v.get_initial_and_last_values().unwrap();

        assert_eq!(1, *l);
        assert_eq!(2, *r);
    }

    #[test]
    fn miri_diff_tree_total() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok(1)).unwrap();

        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        map.apply_to_all_updated_elements::<_, ()>(|l, r, k| {
            assert_eq!(1, *l);
            assert_eq!(2, *r);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn miri_commit_1() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        map.snapshot();

        map.get_or_insert::<()>(&1, || Ok(1)).unwrap();

        map.commit();

        map.apply_to_all_updated_elements::<_, ()>(|_, _, _| {
            panic!("No changes were made.");
        })
        .unwrap();
    }

    #[test]
    fn miri_commit_2() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok(1)).unwrap();

        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        map.commit();

        map.apply_to_all_updated_elements::<_, ()>(|l, r, k| {
            assert_eq!(1, *l);
            assert_eq!(2, *r);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn miri_commit_3() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok(1)).unwrap();

        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok(4)).unwrap();

        v.update::<_, ()>(|x| {
            *x = 3;
            Ok(())
        })
        .unwrap();

        map.commit();

        map.apply_to_all_updated_elements::<_, ()>(|l, r, k| {
            assert_eq!(1, *l);
            assert_eq!(3, *r);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn miri_rollback() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok(1)).unwrap();

        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        let ss = map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok(4)).unwrap();

        v.update::<_, ()>(|x| {
            *x = 3;
            Ok(())
        })
        .unwrap();

        map.snapshot();

        map.rollback(ss).expect("Correct snapshot");

        map.apply_to_all_updated_elements::<_, ()>(|l, r, k| {
            assert_eq!(1, *l);
            assert_eq!(2, *r);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn miri_rollback_reuse() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok(1)).unwrap();

        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        // We'll rollback to this point.
        let ss = map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok(4)).unwrap();

        // This snapshot will be rollbacked.
        v.update::<_, ()>(|x| {
            *x = 3;
            Ok(())
        })
        .unwrap();

        // Just for fun.
        map.snapshot();

        map.rollback(ss).expect("Correct snapshot");

        let mut v = map.get_or_insert::<()>(&1, || Ok(5)).unwrap();

        // This will create a new snapshot and will reuse the one that rollbacked.
        v.update::<_, ()>(|x| {
            *x = 6;
            Ok(())
        })
        .unwrap();

        map.apply_to_all_updated_elements::<_, ()>(|l, r, k| {
            assert_eq!(1, *l);
            assert_eq!(6, *r);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }
}
