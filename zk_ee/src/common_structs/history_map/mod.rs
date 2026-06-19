//! Contains a key-value map that allows reverting items state.

pub(crate) mod element_with_history;
mod record_pool;

use crate::common_structs::history_map::element_with_history::HistoryRecord;
use crate::internal_error;
use crate::utils::ptr_arena::PtrArena;
use crate::{system::errors::internal::InternalError, utils::stack_linked_list::StackLinkedList};
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use core::{alloc::Allocator, fmt::Debug, marker::PhantomData, ops::Bound, ptr::NonNull};
use element_with_history::ElementWithHistory;
pub(crate) use record_pool::HistoryRecordPool;

/// Number of `ElementWithHistory` slots per arena page. Sized so a page fits
/// within a small handful of cache lines for the K/V types in use (~24-52 B
/// keys, ~32 B head/initial/first/committed pointers, plus optional element
/// properties).
const ELEMENT_PAGE_CAPACITY: usize = 32;

/// Stable-address, stable-provenance storage for `ElementWithHistory` values.
/// Handed-out `ElementPtr`s stay valid for reads and writes across later
/// allocations (see [`PtrArena`]).
type ElementArena<K, V, A, KP> =
    PtrArena<ElementWithHistory<K, V, A, KP>, ELEMENT_PAGE_CAPACITY, A>;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct NopSnapshotId;

impl NopSnapshotId {
    pub fn new() -> Self {
        Self
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct CacheSnapshotId(pub(crate) usize);

impl CacheSnapshotId {
    pub fn new() -> Self {
        Self(0)
    }
    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

/// Stable pointer to an `ElementWithHistory` owned by the arena.
type ElementPtr<K, V, A, KP> = NonNull<ElementWithHistory<K, V, A, KP>>;

/// A key-value map with history. State can be reverted to snapshots.
/// The snapshots are created using `Self::snapshot(...)` method.
///
/// Internally, `ElementWithHistory` values live in an arena (a chain of
/// fixed-size pages) so their addresses are stable. The BTreeMap and the
/// pending-updates list store pointers into that arena. This bypasses
/// `BTreeMap::get(&K)` lookups on rollback/commit/iter-pending paths and
/// amortizes the per-element allocation cost over arena pages.
///
/// Structure:
/// [ keys ] => [ history ] := [ snapshot 0 .. snapshot n ].
pub struct HistoryMap<K, V, A: Allocator + Clone, KP = ()> {
    // Drop order (fields drop top-to-bottom): pointer holders before the storage
    // they point into — `btree`/`state` hold pointers into `elements_arena`,
    // whose elements hold record links into `records_memory_pool`. Defensive
    // today (no `Drop` derefs these links), but sound by construction.
    /// Map from key to pointer into the elements arena.
    btree: BTreeMap<K, ElementPtr<K, V, A, KP>, A>,
    state: HistoryMapState<K, V, A, KP>,
    /// Stable-address storage for `ElementWithHistory` values.
    elements_arena: ElementArena<K, V, A, KP>,
    /// Manages memory allocations for history records, reuses old allocations for optimization
    records_memory_pool: HistoryRecordPool<V, A>,
}

struct HistoryMapState<K, V, A: Allocator + Clone, KP> {
    next_snapshot_id: CacheSnapshotId,
    /// State can't be rolled back further than frozen snapshot id. Useful for transactions boundaries
    frozen_snapshot_id: CacheSnapshotId,
    /// Chronological list of pointers to elements updated since the last commit.
    pending_updated_elements: StackLinkedList<(ElementPtr<K, V, A, KP>, CacheSnapshotId), A>,
    alloc: A,
}

impl<K, V, A, KP> HistoryMap<K, V, A, KP>
where
    K: Ord + Clone,
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
            records_memory_pool: HistoryRecordPool::new(alloc.clone()),
            elements_arena: PtrArena::new_in(alloc),
        }
    }

    /// Clears the map while reusing history record allocations.
    pub fn clear(&mut self) {
        for (_, ptr) in self.btree.iter_mut() {
            // Safety: each pointer was produced by `elements_arena.push` and the
            // arena is still alive here. No pending-list user can race with this
            // (we hold `&mut self`).
            let element = unsafe { ptr.as_mut() };
            self.records_memory_pool
                .reuse_memory(element.head, element.initial);
        }
        // Drop the containers that hold arena-derived pointers *before* the
        // arena itself, so the invariant "every pointer in `btree` and in
        // `pending_updated_elements` is valid" holds at every observable
        // point. Defends against any future panic path between the two
        // drops.
        self.btree.clear();
        self.state.pending_updated_elements = StackLinkedList::empty(self.state.alloc.clone());
        // Now safe to release the backing arena pages along with their
        // contained `ElementWithHistory` values (and their owned keys).
        self.elements_arena.clear();
        self.state.next_snapshot_id = CacheSnapshotId(1);
        self.state.frozen_snapshot_id = CacheSnapshotId(0);
    }

    /// Get history of an element by key
    pub fn get(&self, key: &K) -> Option<HistoryMapItemRef<'_, K, V, A, KP>> {
        self.btree.get(key).map(|ptr| HistoryMapItemRef {
            // Safety: pointer is valid for the lifetime of `&self`.
            history: unsafe { ptr.as_ref() },
        })
    }

    /// Get history of an element by key, mutable
    pub fn get_mut(&mut self, key: &K) -> Option<HistoryMapItemRefMut<'_, K, V, A, KP>> {
        let ptr = *self.btree.get(key)?;
        Some(HistoryMapItemRefMut {
            // Pointer is valid for the lifetime of `&mut self`. We carry the raw
            // arena pointer rather than re-entering the BTreeMap so that
            // `cache_state` and `records_memory_pool` can be borrowed mutably
            // alongside.
            element: ptr,
            cache_state: &mut self.state,
            records_memory_pool: &mut self.records_memory_pool,
            _element_borrow: PhantomData,
        })
    }

    /// Get history of an element by key or use callback to insert initial value
    pub fn get_or_insert<E>(
        &mut self,
        key: &K,
        spawn_v: impl FnOnce() -> Result<(V, KP), E>,
    ) -> Result<HistoryMapItemRefMut<'_, K, V, A, KP>, E> {
        let ptr = match self.btree.entry(key.clone()) {
            Entry::Occupied(o) => *o.into_mut(),
            Entry::Vacant(vacant_entry) => {
                let (v, properties) = spawn_v()?;
                let element = ElementWithHistory::new(
                    key.clone(),
                    properties,
                    v,
                    &mut self.records_memory_pool,
                );
                let ptr = self.elements_arena.push(element);
                *vacant_entry.insert(ptr)
            }
        };

        Ok(HistoryMapItemRefMut {
            // Pointer is valid for the lifetime of `&mut self`.
            element: ptr,
            cache_state: &mut self.state,
            records_memory_pool: &mut self.records_memory_pool,
            _element_borrow: PhantomData,
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
                Some((mut element_ptr, update_snapshot_id)) => {
                    // The items in the address_snapshot_updates are ordered chronologically.
                    if update_snapshot_id <= snapshot_id {
                        self.state
                            .pending_updated_elements
                            .push((element_ptr, update_snapshot_id));
                        break;
                    }

                    // Safety: pointer remains valid until `clear()` releases
                    // the arena; the pending list is cleared whenever the
                    // arena is.
                    let item = unsafe { element_ptr.as_mut() };
                    item.rollback(&mut self.records_memory_pool, snapshot_id);

                    node = self.state.pending_updated_elements.pop();
                }
            }
        }

        Ok(())
    }

    /// Commits (freezes) changes up to this point and frees memory taken by snapshots that can't be
    /// rolled back to.
    pub fn commit(&mut self) {
        self.state.frozen_snapshot_id = self.snapshot();

        // Go over all elements changed since last `commit` and `commit` their history
        for (element_ptr, _) in self.state.pending_updated_elements.iter() {
            // Safety: pointer is stable; no live &mut to the element exists.
            let item = unsafe { &mut *element_ptr.as_ptr() };
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
        for (k, ptr) in &self.btree {
            // Safety: pointer is valid for the lifetime of `&self`.
            let element = unsafe { ptr.as_ref() };
            if let Some((initial, last)) = element.get_initial_and_last_values() {
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
        F: FnMut(HistoryMapItemRefMut<K, V, A, KP>) -> Result<(), InternalError>,
    {
        for (_k, ptr) in self.btree.range_mut(range) {
            do_fn(HistoryMapItemRefMut {
                // Pointer is valid for the lifetime of `&mut self`.
                element: *ptr,
                cache_state: &mut self.state,
                records_memory_pool: &mut self.records_memory_pool,
                _element_borrow: PhantomData,
            })?
        }

        Ok(())
    }

    /// Iterate over all elements in map
    pub fn iter(
        &'_ self,
    ) -> impl ExactSizeIterator<Item = HistoryMapItemRef<'_, K, V, A, KP>> + Clone {
        self.btree.values().map(|ptr| HistoryMapItemRef {
            // Safety: pointer is valid for the lifetime of `&self`.
            history: unsafe { ptr.as_ref() },
        })
    }

    /// Iterate over all elements that changed since last commit
    pub fn iter_altered_since_commit(
        &'_ self,
    ) -> impl Iterator<Item = HistoryMapItemRef<'_, K, V, A, KP>> {
        self.state
            .pending_updated_elements
            .iter()
            .map(|(ptr, _)| HistoryMapItemRef {
                // Safety: pointer is valid for the lifetime of `&self`.
                history: unsafe { ptr.as_ref() },
            })
    }

    /// Iterate over the head of each element altered since last commit
    pub fn apply_to_last_record_of_pending_changes<F>(
        &mut self,
        mut do_fn: F,
    ) -> Result<(), InternalError>
    where
        F: FnMut(
            &K,
            (&HistoryRecord<V>, &mut HistoryRecord<V>),
            &mut KP,
        ) -> Result<(), InternalError>,
    {
        for (ptr, _) in self.state.pending_updated_elements.iter() {
            // Safety: stable pointer to an arena-owned ElementWithHistory.
            let record = unsafe { &mut *ptr.as_ptr() };
            let key = &record.key;
            let initial = unsafe { record.initial.as_ref() };
            let current = unsafe { record.head.as_mut() };
            let cache_appearance = &mut record.element_properties;
            do_fn(key, (initial, current), cache_appearance)?
        }

        Ok(())
    }
}

/// External reference to element's history
pub struct HistoryMapItemRef<'a, K, V, A: Allocator + Clone, KP = ()> {
    history: &'a ElementWithHistory<K, V, A, KP>,
}

impl<'a, K, V, A, KP> HistoryMapItemRef<'a, K, V, A, KP>
where
    A: Allocator + Clone,
{
    pub fn key(&self) -> &'a K {
        &self.history.key
    }

    pub fn key_properties(&self) -> &'a KP {
        &self.history.element_properties
    }

    pub fn current(&self) -> &'a V {
        unsafe { &self.history.head.as_ref().value }
    }

    pub fn initial(&self) -> &'a V {
        unsafe { &self.history.initial.as_ref().value }
    }

    pub fn committed(&self) -> &V {
        unsafe { &self.history.committed.as_ref().value }
    }

    /// Returns (initial_value, current_value) if any
    pub fn get_initial_and_last_values(&self) -> Option<(&'a V, &'a V)> {
        self.history.get_initial_and_last_values()
    }
}

/// External mutable reference to element's history
pub struct HistoryMapItemRefMut<'a, K, V, A: Allocator + Clone, KP = ()> {
    /// Canonical arena pointer to the element — the *same* pointer the BTreeMap
    /// stores. We keep the raw pointer (rather than a `&mut`) so that the
    /// pointer pushed into the pending-updates list shares the arena's
    /// provenance: a fresh `&mut`-derived pointer would be invalidated before
    /// the later commit/rollback writes through it. Borrowed for `'a` via the
    /// `&'a mut` fields below (and the marker).
    element: NonNull<ElementWithHistory<K, V, A, KP>>,
    cache_state: &'a mut HistoryMapState<K, V, A, KP>,
    records_memory_pool: &'a mut HistoryRecordPool<V, A>,
    _element_borrow: PhantomData<&'a mut ElementWithHistory<K, V, A, KP>>,
}

impl<'a, K, V, A, KP> HistoryMapItemRefMut<'a, K, V, A, KP>
where
    V: Clone,
    A: Allocator + Clone,
{
    pub fn current(&self) -> &V {
        // Safety: `element` is a valid arena pointer borrowed for `'a`; each
        // access goes through a transient borrow tied to `&self`.
        unsafe { &self.element.as_ref().head.as_ref().value }
    }

    pub fn initial(&self) -> &V {
        unsafe { &self.element.as_ref().initial.as_ref().value }
    }

    pub fn committed(&self) -> &V {
        unsafe { &self.element.as_ref().committed.as_ref().value }
    }

    pub fn element_properties(&self) -> &KP {
        unsafe { &self.element.as_ref().element_properties }
    }

    pub fn element_properties_mut(&mut self) -> &mut KP {
        unsafe { &mut self.element.as_mut().element_properties }
    }

    #[allow(dead_code)]
    /// Returns (initial_value, current_value) if any
    pub fn get_initial_and_last_values(&self) -> Option<(&V, &V)> {
        unsafe { self.element.as_ref() }.get_initial_and_last_values()
    }

    #[must_use]
    /// Use callback `f` to add new record and update element
    pub fn update<F, E>(&mut self, f: F) -> Result<(), E>
    where
        F: FnOnce(&mut V) -> Result<(), E>,
    {
        // Copy of the current head link; the record lives in the records pool,
        // a separate allocation from the element, so transient borrows of one
        // don't conflict with `&mut`-borrows of the other.
        let head_link = unsafe { self.element.as_ref() }.head;

        if unsafe { head_link.as_ref() }.touch_ss_id == self.cache_state.next_snapshot_id {
            // We're in the context of the current snapshot: there are changes that we will simply override
            let head_record = unsafe { &mut *head_link.as_ptr() };
            f(&mut head_record.value)
        } else {
            // The item was last updated before the current snapshot.

            let mut new = self.records_memory_pool.create_record(
                unsafe { head_link.as_ref() }.value.clone(),
                Some(head_link),
                self.cache_state.next_snapshot_id,
            );

            unsafe {
                f(&mut new.as_mut().value)?;
            }

            unsafe { self.element.as_mut() }.add_new_record(new);

            // Push the *canonical* arena pointer (the one the BTreeMap holds):
            // it shares the arena's provenance, so it stays valid for the writes
            // performed later by `commit`/`rollback`. Valid until
            // `HistoryMap::clear`, which also resets the pending list.
            self.cache_state
                .pending_updated_elements
                .push((self.element, self.cache_state.next_snapshot_id));

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

        let v = map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();

        assert_eq!(1, *v.current());
    }

    #[test]
    fn miri_diff_elem_total() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();

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

        let mut v = map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();

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

        map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();

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

        let mut v = map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();

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

        let mut v = map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();

        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok((4, ()))).unwrap();

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

        let mut v = map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();

        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        let ss = map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok((4, ()))).unwrap();

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

        let mut v = map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();

        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        // We'll rollback to this point.
        let ss = map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok((4, ()))).unwrap();

        // This snapshot will be rolled back.
        v.update::<_, ()>(|x| {
            *x = 3;
            Ok(())
        })
        .unwrap();

        // Just for fun.
        map.snapshot();

        map.rollback(ss).expect("Correct snapshot");

        let mut v = map.get_or_insert::<()>(&1, || Ok((5, ()))).unwrap();

        // This will create a new snapshot and will reuse the one that rolled back.
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

    #[test]
    fn clear_removes_elements_and_pending_changes() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        map.snapshot();

        // Create one modified entry.
        let mut v = map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();
        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        assert_eq!(map.iter().len(), 1);
        assert_eq!(map.iter_altered_since_commit().count(), 1);

        // Drop all state.
        map.clear();

        assert!(map.get(&1).is_none());
        assert_eq!(map.iter().len(), 0);
        assert_eq!(map.iter_altered_since_commit().count(), 0);
        map.apply_to_all_updated_elements::<_, ()>(|_, _, _| {
            panic!("Map is expected to be empty after clear")
        })
        .unwrap();
    }

    #[test]
    fn clear_resets_snapshots() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        // Keep a pre-clear snapshot handle.
        let pre_clear_snapshot = map.snapshot();

        let mut v = map.get_or_insert::<()>(&1, || Ok((1, ()))).unwrap();
        v.update::<_, ()>(|x| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        map.clear();

        // Old snapshot ids are no longer valid.
        assert!(map.rollback(pre_clear_snapshot).is_err());

        // Materialize key after clear with initial value.
        map.get_or_insert::<()>(&1, || Ok((3, ()))).unwrap();

        // Take snapshot after clear.
        let post_clear_snapshot = map.snapshot();
        assert_eq!(post_clear_snapshot, super::CacheSnapshotId(1));

        let mut v = map.get_or_insert::<()>(&1, || Ok((5, ()))).unwrap();
        v.update::<_, ()>(|x| {
            *x = 4;
            Ok(())
        })
        .unwrap();

        // Rollback restores the post-clear initial value for this key.
        map.rollback(post_clear_snapshot).expect("Valid snapshot");
        let restored = map.get(&1).expect("Element must remain after rollback");

        assert_eq!(*restored.initial(), 3);
        assert_eq!(*restored.current(), 3);
    }

    /// Regression test for the arena variant: pointers stored in the BTreeMap and
    /// the pending-updates list must stay valid across `PtrArena` page appends.
    ///
    /// The other unit tests only ever insert a single key, so they live entirely
    /// within the first arena page and never trigger a page append. Here we insert
    /// well over one page worth of keys, then update entries on *both* the first
    /// page (small keys) and the latest pages (large keys): if a page append had
    /// invalidated an earlier page's pointer, those entries would observe a stale
    /// or wrong value. Finally we roll back, re-apply + commit, and clear — each
    /// path walks pointers into every page.
    #[test]
    fn spans_multiple_arena_pages() {
        use super::ELEMENT_PAGE_CAPACITY;

        // Span several pages, with a partially-filled final page (the `+ 1`),
        // so both the same-page and new-page branches of the arena's `push` are
        // hit. Derived from the real capacity so the test keeps spanning
        // multiple pages if the constant is retuned.
        let count = ELEMENT_PAGE_CAPACITY * 3 + 1;

        let mut map = HistoryMap::<usize, usize, Global>::new(Global);

        // Initial values: key k -> k, materialized across many arena pages.
        for k in 0..count {
            map.get_or_insert::<()>(&k, || Ok((k, ()))).unwrap();
        }

        // Snapshot we will roll back to.
        let ss = map.snapshot();

        // Update every entry (old and new pages alike). A page append that had
        // invalidated an earlier page's pointer would make the small-k writes
        // here land in the wrong slot, which the assertions below would catch.
        for k in 0..count {
            let mut v = map.get_mut(&k).expect("key present");
            v.update::<_, ()>(|x| {
                *x = k + 1000;
                Ok(())
            })
            .unwrap();
        }

        // Every entry, on every page, reflects its update.
        for k in 0..count {
            let item = map.get(&k).expect("key present");
            assert_eq!(*item.initial(), k);
            assert_eq!(*item.current(), k + 1000);
        }

        // Roll back the bulk update; every entry returns to its initial value,
        // proving the pending-list pointers into every page were followed.
        map.rollback(ss).expect("valid snapshot");
        for k in 0..count {
            assert_eq!(*map.get(&k).expect("key present").current(), k);
        }
        // Nothing remains pending after a full rollback.
        map.apply_to_all_updated_elements::<_, ()>(|_, _, _| {
            panic!("all updates were rolled back");
        })
        .unwrap();

        // Re-apply across all pages and commit; committed values must stick.
        map.snapshot();
        for k in 0..count {
            let mut v = map.get_mut(&k).expect("key present");
            v.update::<_, ()>(|x| {
                *x = k + 2000;
                Ok(())
            })
            .unwrap();
        }
        map.commit();
        for k in 0..count {
            let item = map.get(&k).expect("key present");
            assert_eq!(*item.committed(), k + 2000);
            assert_eq!(*item.current(), k + 2000);
        }

        // Clear releases every page; afterwards the map is empty and reusable.
        map.clear();
        assert_eq!(map.iter().len(), 0);
        for k in 0..count {
            assert!(map.get(&k).is_none());
        }
    }

    /// Exercises `for_each_range`, which hands out a `HistoryMapItemRefMut` built
    /// from a raw arena pointer and (via `update`) pushes that same pointer into
    /// the pending list and writes through it. The range spans multiple arena
    /// pages, so under Miri this checks the range-walk + write path keeps
    /// consistent pointer provenance across page boundaries.
    #[test]
    fn miri_for_each_range() {
        use super::ELEMENT_PAGE_CAPACITY;
        use crate::system::errors::internal::InternalError;
        use core::ops::Bound;

        // Several pages, partially-filled last page.
        let count = ELEMENT_PAGE_CAPACITY * 3 + 1;
        // Sub-range that starts on the first page and ends on the last one.
        let lo = 1usize;
        let hi = count - 2;

        let mut map = HistoryMap::<usize, usize, Global>::new(Global);
        for k in 0..count {
            map.get_or_insert::<()>(&k, || Ok((k, ()))).unwrap();
        }
        map.snapshot();

        // Mutate the [lo, hi] sub-range through the mutable handle.
        map.for_each_range((Bound::Included(&lo), Bound::Included(&hi)), |mut item| {
            let cur = *item.current();
            item.update::<_, InternalError>(|x| {
                *x = cur + 100;
                Ok(())
            })?;
            Ok(())
        })
        .unwrap();

        // In-range keys (across every page) updated; out-of-range keys untouched.
        for k in 0..count {
            let item = map.get(&k).expect("key present");
            let expected = if (lo..=hi).contains(&k) { k + 100 } else { k };
            assert_eq!(*item.current(), expected);
        }
    }

    /// Exercises `iter_altered_since_commit`, which reads each element through a
    /// `NonNull` taken from the pending-updates list (`ptr.as_ref()`). Under Miri
    /// this guards those foreign reads against invalidating the live arena.
    #[test]
    fn miri_iter_altered_since_commit() {
        let mut map = HistoryMap::<usize, usize, Global>::new(Global);
        for k in 0..4usize {
            map.get_or_insert::<()>(&k, || Ok((k, ()))).unwrap();
        }
        map.snapshot();

        // Alter only keys 1 and 2 — exactly these should be reported as altered.
        for k in [1usize, 2] {
            let mut v = map.get_mut(&k).expect("key present");
            v.update::<_, ()>(|x| {
                *x = k + 10;
                Ok(())
            })
            .unwrap();
        }

        let mut seen: std::vec::Vec<(usize, usize)> = map
            .iter_altered_since_commit()
            .map(|item| (*item.key(), *item.current()))
            .collect();
        seen.sort();
        assert_eq!(seen, std::vec![(1usize, 11usize), (2, 12)]);
    }

    /// Exercises `apply_to_last_record_of_pending_changes` with a non-`()` `KP`,
    /// the path that derives `&initial` / `&mut head` / `&mut properties` all
    /// from a single `&mut` to one arena element. Also drives
    /// `element_properties` / `element_properties_mut`. The updated elements are
    /// spread across multiple arena pages, and this overlapping-borrow shape is
    /// the strictest case for Tree Borrows.
    #[test]
    fn miri_apply_to_last_record_with_properties() {
        use super::ELEMENT_PAGE_CAPACITY;

        // Span several pages; update every third key so altered elements land on
        // every page (and `even` keys verify the untouched path too).
        let count = ELEMENT_PAGE_CAPACITY * 3 + 1;
        let altered = |k: usize| k % 3 == 0;

        // KP = u32 so the property paths are actually exercised.
        let mut map = HistoryMap::<usize, usize, Global, u32>::new(Global);
        for k in 0..count {
            map.get_or_insert::<()>(&k, || Ok((k, k as u32))).unwrap();
        }
        map.snapshot();

        // Update value and properties through a RefMut on the altered subset.
        for k in (0..count).filter(|&k| altered(k)) {
            let mut item = map.get_mut(&k).expect("key present");
            item.update::<_, ()>(|x| {
                *x = k + 100;
                Ok(())
            })
            .unwrap();
            *item.element_properties_mut() += 1;
            assert_eq!(*item.element_properties(), k as u32 + 1);
        }

        // Walk the pending heads: read `initial`, mutate `current` and the
        // properties through the aliased references.
        map.apply_to_last_record_of_pending_changes(|key, (initial, current), props| {
            assert_eq!(initial.value, *key);
            assert_eq!(current.value, *key + 100);
            current.value += 1;
            *props += 10;
            Ok(())
        })
        .unwrap();

        // Mutations through the aliased refs are visible on every page; untouched
        // keys keep their initial value/properties.
        for k in 0..count {
            let item = map.get(&k).expect("key present");
            if altered(k) {
                assert_eq!(*item.current(), k + 101);
                assert_eq!(*item.key_properties(), k as u32 + 11);
            } else {
                assert_eq!(*item.current(), k);
                assert_eq!(*item.key_properties(), k as u32);
            }
        }
    }
}
