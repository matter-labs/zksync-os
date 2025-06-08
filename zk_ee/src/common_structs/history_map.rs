//! Contains a key-value map that allows reverting items state.
use alloc::boxed::Box;

use crate::{system::errors::InternalError, utils::stack_linked_list::StackLinkedList};
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use core::{alloc::Allocator, fmt::Debug, ops::Bound, ptr::NonNull};

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
    btree: BTreeMap<K, ElementHistory<V, A>, A>,
    state: HistoryMapState<K, A>,
    /// Manages memory allocations for history records, reuses old allocations for optimization
    records_memory_pool: ElementSource<V, A>,
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
            records_memory_pool: ElementSource::new(alloc),
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
        // TODO: we clone key (32+ bytes in some cases) for every access
        let entry = self.btree.entry(key.clone());

        let v = match entry {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(vacant_entry) => {
                let v = spawn_v()?;
                vacant_entry.insert(ElementHistory::new(
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
            return Err(InternalError("History map: rollback below frozen snapshot"));
        }

        if snapshot_id >= self.state.next_snapshot_id {
            return Err(InternalError(
                "History map: rollback to non-existent snapshot",
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
    /// TODO rename to reset or smth
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

    // TODO check usage
    /// Applies callback `do_fn` to all pairs (initial_value, current_value)
    pub fn for_total_diff_operands<F, E>(&self, mut do_fn: F) -> Result<(), E>
    where
        F: FnMut(&V, &V, &K) -> Result<(), E>,
    {
        for (k, v) in &self.btree {
            if let Some((l, r)) = v.diff_operands_total() {
                do_fn(l, r, k)?;
            }
        }

        Ok(())
    }

    // TODO used only to cleanup in storage (reset appearance)
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

    // TODO used only for new preimages publication storage
    pub fn iter(&self) -> impl Iterator<Item = HistoryMapItemRef<'_, K, V, A>> + Clone {
        self.btree
            .iter()
            .map(|(k, v)| HistoryMapItemRef { key: k, history: v })
    }

    // TODO used only for account cache
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
}

type HistoryRecordLink<V> = NonNull<HistoryRecord<V>>;

/// Record in some element's history
struct HistoryRecord<V> {
    touch_ss_id: CacheSnapshotId,
    value: V,
    previous: Option<HistoryRecordLink<V>>,
}

/// The history linked list. Always has at least one item with the snapshot id of 0.
pub(crate) struct ElementHistory<V, A: Allocator + Clone> {
    /// Initial record (before history started)
    initial: HistoryRecordLink<V>,
    first: HistoryRecordLink<V>,
    /// Current history record
    head: HistoryRecordLink<V>,
    alloc: A,
}

impl<V, A: Allocator + Clone> Drop for ElementHistory<V, A> {
    fn drop(&mut self) {
        let mut elem = unsafe { Box::from_raw_in(self.head.as_ptr(), self.alloc.clone()) };

        while let Some(n) = elem.previous.take() {
            let n = unsafe { Box::from_raw_in(n.as_ptr(), self.alloc.clone()) };

            elem = n;
        } // `n` is dropped here.
    } // last elem is dropped here.
}

impl<V, A: Allocator + Clone> ElementHistory<V, A> {
    #[inline(always)]
    fn new(value: V, records_memory_pool: &mut ElementSource<V, A>, alloc: A) -> Self {
        // Note: initial value always has snapshot id 0
        let elem = records_memory_pool.create_element(value, None, CacheSnapshotId(0));

        Self {
            head: elem,
            initial: elem,
            first: elem,
            alloc,
        }
    }

    /// Rollback element's state to snapshot_id
    /// Removed history records stored in records_memory_pool to reuse later
    fn rollback(
        &mut self,
        records_memory_pool: &mut ElementSource<V, A>,
        snapshot_id: CacheSnapshotId,
    ) {
        // Caller should guarantee that snapshot_id is correct

        if unsafe { self.head.as_ref() }.touch_ss_id <= snapshot_id {
            return;
        }

        let mut first_removed_record = self.head;
        // Find first elem such that elem.touch_ss_id > snapshot_id and set previous as first_removed_record
        loop {
            let n_lnk = unsafe {
                first_removed_record
                    .as_mut()
                    .previous
                    .as_mut()
                    .expect("Every history is terminated with a 0'th snapshot")
            };

            let n = unsafe { n_lnk.as_mut() };

            if n.touch_ss_id <= snapshot_id {
                // This is guaranteed to happen by encountering the terminator snapshot.
                break;
            }

            first_removed_record = *n_lnk;
        }

        let last_removed_record = self.head;

        let new_head = unsafe { first_removed_record.as_mut() }
            .previous
            .take()
            .unwrap();

        if first_removed_record == self.first {
            self.first = new_head;
        }

        self.head = new_head;

        // Return subchain to the pool to be reused later
        records_memory_pool.recycle_memory(last_removed_record, first_removed_record);
    }

    /// Returns (initial_value, current_value) if any
    fn diff_operands_total(&self) -> Option<(&V, &V)> {
        let entry = unsafe { self.head.as_ref() };
        match entry.previous {
            None => None,
            Some(_) => Some((unsafe { &self.initial.as_ref().value }, &entry.value)),
        }
    }

    /// Commits (freezes) changes up to this point
    /// Frees memory taken by snapshots that can't be rollbacked to.
    fn commit(&mut self, records_memory_pool: &mut ElementSource<V, A>) {
        // Case with only initial value (no writes at all)
        if self.head == self.initial {
            return;
        }

        // Current snapshot is the one we're committing to (only one update).
        if self.head == self.first {
            return;
        }

        // Safety: initial and first elements are distinct. Cases with 0-1 updates are covered above.

        let first_removed_record = self.first;

        // Previous head becomes new `first` record
        self.first = self.head;

        let head_mut = unsafe { self.head.as_mut() };
        let last_removed_record = head_mut
            .previous
            .replace(self.initial)
            .expect("History has at least 3 items.");

        // Return subchain to the pool to be reused later
        records_memory_pool.recycle_memory(last_removed_record, first_removed_record);
    }
}

/// External reference to element's history
pub struct HistoryMapItemRef<'a, K: Clone, V, A: Allocator + Clone> {
    key: &'a K,
    history: &'a ElementHistory<V, A>,
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
    pub fn diff_operands_total(&self) -> Option<(&V, &V)> {
        self.history.diff_operands_total()
    }
}

/// External mutable reference to element's history
pub struct HistoryMapItemRefMut<'a, K: Clone, V, A: Allocator + Clone> {
    history: &'a mut ElementHistory<V, A>,
    cache_state: &'a mut HistoryMapState<K, A>,
    records_memory_pool: &'a mut ElementSource<V, A>,
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
    pub fn diff_operands_total(&self) -> Option<(&V, &V)> {
        self.history.diff_operands_total()
    }

    #[must_use]
    /// Use callback `f` to add new record and update element
    pub fn update<F, E>(&mut self, f: F) -> Result<(), E>
    where
        F: FnOnce(&mut V) -> Result<(), E>,
    {
        let history = unsafe { self.history.head.as_mut() };

        if history.touch_ss_id == self.cache_state.next_snapshot_id {
            // We're in the context of the current snapshot: there are changes that we will simply override
            f(&mut history.value)
        } else {
            // The item was last updated before the current snapshot.

            let mut new = self.records_memory_pool.create_element(
                history.value.clone(), // TODO: cloning value
                Some(self.history.head),
                self.cache_state.next_snapshot_id,
            );

            unsafe {
                f(&mut new.as_mut().value)?;
            }

            self.history.head = new;
            if self.history.initial == self.history.first {
                // When don't have any updates before
                self.history.first = new;
            }

            self.cache_state
                .pending_updated_elements
                .push((self.key.clone(), self.cache_state.next_snapshot_id));

            Ok(())
        }
    }
}

// TODO: can be optimized using arena-like allocation strategy
/// Manages memory allocations for history records, reuses old allocations for optimization
struct ElementSource<V, A: Allocator + Clone> {
    /// Head of `recycled` sub-list
    head: Option<HistoryRecordLink<V>>,
    /// Tail of `recycled` sub-list
    last: Option<HistoryRecordLink<V>>,
    alloc: A,
}

impl<V, A: Allocator + Clone> Drop for ElementSource<V, A> {
    fn drop(&mut self) {
        if let Some(head) = self.head {
            let mut elem = unsafe { Box::from_raw_in(head.as_ptr(), self.alloc.clone()) };

            while let Some(n) = elem.previous.take() {
                let n = unsafe { Box::from_raw_in(n.as_ptr(), self.alloc.clone()) };

                elem = n;
            } // `n` is dropped here.
        } // Last elem is dropped here.
    }
}

impl<V, A: Allocator + Clone> ElementSource<V, A> {
    fn new(alloc: A) -> Self {
        Self {
            head: Default::default(),
            last: Default::default(),
            alloc,
        }
    }

    /// Allocate memory or reuse old record and create a new record
    fn create_element(
        &mut self,
        value: V,
        previous: Option<HistoryRecordLink<V>>,
        snapshot_id: CacheSnapshotId,
    ) -> HistoryRecordLink<V> {
        match self.head {
            None => {
                // Allocate
                let raw = Box::into_raw(Box::new_in(
                    HistoryRecord {
                        touch_ss_id: snapshot_id,
                        value,
                        previous,
                    },
                    self.alloc.clone(),
                ));
                // Safety: `Box::into_raw` pinky swears that the ptr is non null and properly
                // aligned.
                unsafe { NonNull::new_unchecked(raw) }
            }
            Some(mut elem) => {
                // Reuse old allocation
                {
                    let elem = unsafe { elem.as_mut() };

                    self.head = elem.previous.take();

                    if self.head.is_none() {
                        self.last = None;
                    }

                    // Safety: We *must* rewrite all the links in `elem`.
                    elem.touch_ss_id = snapshot_id;
                    elem.value = value;
                    elem.previous = previous;
                }

                elem
            }
        }
    }

    /// Store a chain of records to reuse them later
    fn recycle_memory(
        &mut self,
        chain_head: HistoryRecordLink<V>,
        mut chain_tail: HistoryRecordLink<V>,
    ) {
        match self.last {
            None => {
                self.head = Some(chain_head);
            }
            Some(ref mut last) => {
                unsafe { last.as_mut().previous = Some(chain_head) };
            }
        }

        // We need to unlink this, cause it still points to the original history it's been taken
        // from.
        unsafe { chain_tail.as_mut().previous = None };

        self.last = Some(chain_tail);
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

        let (l, r) = v.diff_operands_total().unwrap();

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

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
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

        map.for_total_diff_operands::<_, ()>(|_, _, _| {
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

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
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

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
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

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
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

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
            assert_eq!(1, *l);
            assert_eq!(6, *r);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }
}
