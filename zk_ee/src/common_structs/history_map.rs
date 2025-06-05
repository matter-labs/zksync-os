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

type HistoryRecordLink<V> = NonNull<HistoryRecord<V>>;

struct HistoryRecord<V> {
    touch_ss_id: CacheSnapshotId,
    value: V,
    previous: Option<HistoryRecordLink<V>>,
}

/// The history linked list. Always has at least one item with the snapshot id of 0.
pub(crate) struct ElementHistory<V, A: Allocator + Clone> {
    initial: HistoryRecordLink<V>,
    first: HistoryRecordLink<V>,
    head: HistoryRecordLink<V>,
    alloc: A,
}

pub struct HistoryMapItemRef<'a, K: Clone, V, A: Allocator + Clone> {
    key: &'a K,
    history: &'a ElementHistory<V, A>,
}

/// Returned to the user to manipulate and access the history.
pub struct HistoryMapItemRefMut<'a, K: Clone, V, A: Allocator + Clone> {
    history: &'a mut ElementHistory<V, A>,
    cache_state: &'a mut HistoryMapState<K, A>,
    source: &'a mut ElementSource<V, A>,
    key: &'a K,
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
    fn new(value: V, source: &mut ElementSource<V, A>, alloc: A) -> Self {
        let elem = source.get(value, None, CacheSnapshotId(0));

        Self {
            head: elem,
            initial: elem,
            first: elem,
            alloc,
        }
    }

    fn rollback(&mut self, reuse: &mut ElementSource<V, A>, snapshot_id: CacheSnapshotId) {
        // Caller should guarantee that snapshot_id > 0

        if unsafe { self.head.as_ref() }.touch_ss_id <= snapshot_id {
            return;
        }

        // Find first elem such that elem.touch_ss_id >= snapshot_id
        let mut first_removed_record = self.head;
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

        reuse.put_back(last_removed_record, first_removed_record);
    }

    fn diff_operands_total(&self) -> Option<(&V, &V)> {
        let entry = unsafe { self.head.as_ref() };
        match entry.previous {
            None => None,
            Some(_) => Some((unsafe { &self.initial.as_ref().value }, &entry.value)),
        }
    }

    fn commit(&mut self, reuse: &mut ElementSource<V, A>) {
        // Single snapshot.
        if self.head == self.initial {
            return;
        }

        // Current snapshot is the one we're committing to.
        if self.head == self.first {
            return;
        }

        // Safety: initial and first elements are distinct, because they only are when
        // there's only a single snapshot in the history, a case we've covered above. On an update,
        // the first link will point to correct item.
        //
        // We're removing the non extremities, such that first item becomes the top.

        let freed_end = self.first;
        self.first = self.head;

        let top = unsafe { self.head.as_mut() };
        let freed_start = top
            .previous
            .replace(self.initial)
            .expect("History has at least 3 items.");

        reuse.put_back(freed_start, freed_end);
    }
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

    pub fn diff_operands_total(&self) -> Option<(&V, &V)> {
        self.history.diff_operands_total()
    }
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
    pub fn diff_operands_total(&self) -> Option<(&V, &V)> {
        self.history.diff_operands_total()
    }

    #[must_use]
    pub fn update<F, E>(&mut self, f: F) -> Result<(), E>
    where
        F: FnOnce(&mut V) -> Result<(), E>,
    {
        let history = unsafe { self.history.head.as_mut() };

        if history.touch_ss_id == self.cache_state.current_snapshot_id {
            // We're in the context of the current stapshot.
            f(&mut history.value)
        } else {
            // The item was last updated before the current snapshot.

            let mut new = self.source.get(
                history.value.clone(),
                Some(self.history.head),
                self.cache_state.current_snapshot_id,
            );

            unsafe {
                f(&mut new.as_mut().value)?;
            }

            self.history.head = new;
            if self.history.initial == self.history.first {
                // When we have a single item.
                self.history.first = new;
            }

            self.cache_state
                .updated_elems
                .push((self.key.clone(), self.cache_state.current_snapshot_id));

            Ok(())
        }
    }
}

struct ElementSource<V, A: Allocator + Clone> {
    head: Option<HistoryRecordLink<V>>,
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

    fn get(
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

    fn put_back(&mut self, chain_start: HistoryRecordLink<V>, mut chain_end: HistoryRecordLink<V>) {
        match self.last {
            None => {
                self.head = Some(chain_start);
            }
            Some(ref mut last) => {
                unsafe { last.as_mut().previous = Some(chain_start) };
            }
        }

        // We need to unlink this, cause it still points to the original history it's been taken
        // from.
        unsafe { chain_end.as_mut().previous = None };

        self.last = Some(chain_end);
    }
}

struct HistoryMapState<K, A: Allocator + Clone> {
    current_snapshot_id: CacheSnapshotId,
    frozen_snapshot_id: CacheSnapshotId,
    updated_elems: StackLinkedList<(K, CacheSnapshotId), A>,
    alloc: A,
}

/// A key-value map that allows to store history of the values and to revert their state. The
/// history is a list of stapshots. The snapshots are created on demand between
/// `Self::snapshot(...)` calls.
///
/// Structure:
/// [ keys ] => [ history ] := [ snapshot 0 .. snapshot n ].
pub struct HistoryMap<K, V, A: Allocator + Clone> {
    btree: BTreeMap<K, ElementHistory<V, A>, A>,
    state: HistoryMapState<K, A>,
    reuse: ElementSource<V, A>,
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
                // All new retreivals are going to id 0 to allow differentiating retreivals with
                // updates in a single snapshot span.
                current_snapshot_id: CacheSnapshotId(1),
                frozen_snapshot_id: CacheSnapshotId(0),
                updated_elems: StackLinkedList::empty(alloc.clone()),
            },
            reuse: ElementSource::new(alloc),
        }
    }

    pub fn get<'s>(&'s mut self, key: &'s K) -> Option<HistoryMapItemRef<'s, K, V, A>> {
        self.btree
            .get(key)
            .map(|ec| HistoryMapItemRef { key, history: ec })
    }

    pub fn get_mut<'s>(&'s mut self, key: &'s K) -> Option<HistoryMapItemRefMut<'s, K, V, A>> {
        self.btree.get_mut(key).map(|ec| HistoryMapItemRefMut {
            key,
            history: ec,
            cache_state: &mut self.state,
            source: &mut self.reuse,
        })
    }

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
                    &mut self.reuse,
                    self.state.alloc.clone(),
                ))
            }
        };

        Ok(HistoryMapItemRefMut {
            key,
            history: v,
            cache_state: &mut self.state,
            source: &mut self.reuse,
        })
    }

    pub fn snapshot(&mut self) -> CacheSnapshotId {
        let current_snapshot_id = self.state.current_snapshot_id;
        self.state.current_snapshot_id.increment();
        current_snapshot_id
    }

    /// Rollbacks the data to the state at the provided `snapshot_id`.
    pub fn rollback(&mut self, snapshot_id: CacheSnapshotId) {
        if snapshot_id < self.state.frozen_snapshot_id {
            // TODO: replace with internal error
            panic!("Rolling below frozen snapshot is illegal and will cause UB.")
        }

        let mut node = self.state.updated_elems.pop();
        loop {
            match node {
                None => break,
                Some((key, update_snapshot_id)) => {
                    // The items in the address_snapshot_updates are ordered chronologically.
                    if update_snapshot_id <= snapshot_id {
                        self.state.updated_elems.push((key, update_snapshot_id));
                        break;
                    }

                    let item = self
                        .btree
                        .get_mut(&key)
                        .expect("We've updated this, so it must be present.");

                    item.rollback(&mut self.reuse, snapshot_id);

                    node = self.state.updated_elems.pop();
                }
            }
        }
    }

    /// Commits changes up to this point and frees memory taken by snapshots that can't be
    /// rollbacked to.
    /// TODO rename to reset or smth
    pub fn commit(&mut self) {
        self.state.frozen_snapshot_id = self.snapshot();

        for (key, _) in self.state.updated_elems.iter() {
            let item = self
                .btree
                .get_mut(key)
                .expect("We've updated this, so it must be present.");

            item.commit(&mut self.reuse);
        }

        // We've committed, so we don't need those changes anymore.
        self.state.updated_elems = StackLinkedList::empty(self.state.alloc.clone());
    }

    // TODO check usage
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

    // TODO used to cleanup in storage
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
                source: &mut self.reuse,
            })?
        }

        Ok(())
    }

    // TODO only for new preimages publication storage
    pub fn iter(&self) -> impl Iterator<Item = HistoryMapItemRef<'_, K, V, A>> + Clone {
        self.btree
            .iter()
            .map(|(k, v)| HistoryMapItemRef { key: k, history: v })
    }

    // TODO use only for account cache
    pub fn iter_altered_since_commit(
        &self,
    ) -> impl Iterator<Item = HistoryMapItemRef<'_, K, V, A>> {
        self.state
            .updated_elems
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

        map.rollback(ss);

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

        map.rollback(ss);

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
