//! Contains a key-value map that allows reverting items state.
use alloc::boxed::Box;

use crate::{system::errors::InternalError, utils::stack_linked_list::StackLinkedList};
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use core::{alloc::Allocator, fmt::Debug, marker::PhantomData, ops::Bound, ptr::NonNull};

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

type HistoryLink<V, A> = NonNull<HistoryItem<V, A>>;

/// The history linked list. Always has at least one item with the snapshot id of 0.
struct HistoryItem<V, A: Allocator> {
    touch_ss_id: CacheSnapshotId,
    value: V,
    next: Option<HistoryLink<V, A>>,
    phantom: PhantomData<(V, A)>,
}

pub(crate) struct ElementContainer<V, A: Allocator + Clone> {
    ultimate: HistoryLink<V, A>,
    penultimate: HistoryLink<V, A>,
    head: HistoryLink<V, A>,
    alloc: A,
}

pub struct HistoryMapItemRef<'a, K: Clone, V, A: Allocator + Clone> {
    key: &'a K,
    container: &'a ElementContainer<V, A>,
}

/// Returned to the user to manipulate and access the history.
pub struct HistoryMapItemRefMut<'a, K: Clone, V, A: Allocator + Clone> {
    container: &'a mut ElementContainer<V, A>,
    cache_state: &'a mut HistoryMapState<K, A>,
    source: &'a mut ElementSource<V, A>,
    key: &'a K,
}

impl<V, A: Allocator + Clone> Drop for ElementContainer<V, A> {
    fn drop(&mut self) {
        let mut elem = unsafe { Box::from_raw_in(self.head.as_ptr(), self.alloc.clone()) };

        while let Some(n) = elem.next.take() {
            let n = unsafe { Box::from_raw_in(n.as_ptr(), self.alloc.clone()) };

            elem = n;
        } // `n` is dropped here.
    } // last elem is dropped here.
}

impl<V, A: Allocator> Debug for HistoryItem<V, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CacheHistoryItem")
            .field("snapshot_id", &self.touch_ss_id)
            .field("tail", &self.next)
            .finish()
    }
}

impl<T, A: Allocator> HistoryItem<T, A> {
    pub fn last(&self) -> &T {
        let mut elem = self;

        while let Some(n) = &elem.next {
            elem = unsafe { &*n.as_ptr() };
        }

        &elem.value
    }

    #[allow(dead_code)]
    pub fn diff_operands_total(&self) -> Option<(&T, &T)> {
        match &self.next {
            None => None,
            Some(next) => {
                let right = &self.value;
                let mut left = next;

                while let Some(next) = &(unsafe { left.as_ref() }).next {
                    left = next;
                }

                Some((&(unsafe { left.as_ref() }).value, right))
            }
        }
    }
}

impl<V, A: Allocator + Clone> ElementContainer<V, A> {
    #[inline(always)]
    fn new(value: V, source: &mut ElementSource<V, A>, alloc: A) -> Self {
        let elem = match source.get() {
            Some(mut elem) => {
                {
                    // Safety:
                    let elem = unsafe { elem.as_mut() };

                    // Safety: We *must* rewrite all the links in `elem`.
                    elem.touch_ss_id = CacheSnapshotId(0);
                    elem.value = value;
                    elem.next = None;
                }
                elem
            }
            None => {
                let raw = Box::into_raw(Box::new_in(
                    HistoryItem {
                        touch_ss_id: CacheSnapshotId(0),
                        value,
                        next: None,
                        phantom: PhantomData,
                    },
                    alloc.clone(),
                ));
                // Safety: `Box::into_raw` pinky swears that the ptr is non null and properly
                // aligned.
                unsafe { NonNull::new_unchecked(raw) }
            }
        };

        Self {
            head: elem,
            ultimate: elem,
            penultimate: elem,
            alloc,
        }
    }

    fn rollback(&mut self, reuse: &mut ElementSource<V, A>, snapshot_id: CacheSnapshotId) {
        if unsafe { self.head.as_ref() }.touch_ss_id < snapshot_id {
            return;
        }

        let mut elem_lnk = self.head;

        loop {
            let n_lnk = unsafe {
                elem_lnk
                    .as_mut()
                    .next
                    .as_mut()
                    .expect("Every history is terminated with a 0'th snapshot")
            };

            let n = unsafe { n_lnk.as_mut() };

            if n.touch_ss_id < snapshot_id {
                // This is guaranteed to happen by encountering the terminator snapshot.

                break;
            }

            elem_lnk = *n_lnk;
        }

        let freed_start = self.head;
        let freed_end = elem_lnk;

        let n_head = unsafe { elem_lnk.as_mut() }.next.take().unwrap();
        let n_h1 = unsafe { n_head.as_ref() }.next;
        let (penultimate, ultimate) = match n_h1 {
            None => (n_head, n_head),
            Some(n_h1) => unsafe { n_h1.as_ref() }
                .next
                .map_or((n_head, n_h1), |n_h2| (n_h1, n_h2)),
        };
        self.head = n_head;
        self.penultimate = penultimate;
        self.ultimate = ultimate;

        reuse.put_back(freed_start, freed_end);
    }

    fn diff_operands_total(&self) -> Option<(&V, &V)> {
        let entry = unsafe { self.head.as_ref() };
        match entry.next {
            None => None,
            Some(_) => Some((unsafe { &self.ultimate.as_ref().value }, &entry.value)),
        }
    }

    fn commit(&mut self, reuse: &mut ElementSource<V, A>) {
        // Single snapshot.
        if self.head == self.ultimate {
            return;
        }

        // Current snapshot is the one we're committing to.
        if self.head == self.penultimate {
            return;
        }

        // Safety: Ultimate and penultimate elements are distinct, because they only are when
        // there's only a single snapshot in the history, a case we've covered above. On an update,
        // the penultimate link will point to correct item.
        //
        // We're removing the non extremities, such that penultimate item becomes the top.

        let freed_end = self.penultimate;
        self.penultimate = self.head;

        let top = unsafe { self.head.as_mut() };
        let freed_start = top
            .next
            .replace(self.ultimate)
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
        unsafe { &self.container.head.as_ref().value }
    }

    pub fn last(&self) -> &V {
        unsafe { &self.container.head.as_ref().last() }
    }

    pub fn diff_operands_total(&self) -> Option<(&V, &V)> {
        self.container.diff_operands_total()
    }
}

impl<'a, K, V, A> HistoryMapItemRefMut<'a, K, V, A>
where
    K: Clone + Debug,
    V: Clone,
    A: Allocator + Clone,
{
    pub fn current(&self) -> &V {
        unsafe { &self.container.head.as_ref().value }
    }

    #[allow(dead_code)]
    pub fn diff_operands_total(&self) -> Option<(&V, &V)> {
        self.container.diff_operands_total()
    }

    #[must_use]
    pub fn update<F, E>(&mut self, f: F) -> Result<(), E>
    where
        F: FnOnce(&mut V) -> Result<(), E>,
    {
        let history = unsafe { self.container.head.as_mut() };

        if history.touch_ss_id == self.cache_state.current_snapshot_id {
            // We're in the context of the current stapshot.
            f(&mut history.value)
        } else {
            // The item was last updated before the current snapshot.

            let mut new = match self.source.get() {
                Some(mut elem) => {
                    {
                        let elem = unsafe { elem.as_mut() };

                        elem.value = history.value.clone();
                        elem.touch_ss_id = self.cache_state.current_snapshot_id;
                        elem.next = Some(self.container.head);
                    }

                    elem
                }
                None => {
                    let item = HistoryItem {
                        value: history.value.clone(),
                        touch_ss_id: self.cache_state.current_snapshot_id,
                        next: Some(self.container.head),
                        phantom: PhantomData,
                    };
                    let raw = Box::into_raw(Box::new_in(item, self.container.alloc.clone()));
                    unsafe { NonNull::new_unchecked(raw) }
                }
            };

            unsafe {
                let new = new.as_mut();
                f(&mut new.value)?;
            }

            self.container.head = new;
            if self.container.ultimate == self.container.penultimate {
                // When we have a single item.
                self.container.penultimate = new;
            }

            self.cache_state
                .updated_elems
                .push((self.key.clone(), self.cache_state.current_snapshot_id));

            Ok(())
        }
    }
}

struct ElementSource<V, A: Allocator + Clone> {
    head: Option<HistoryLink<V, A>>,
    last: Option<HistoryLink<V, A>>,
    alloc: A,
}

impl<V, A: Allocator + Clone> Drop for ElementSource<V, A> {
    fn drop(&mut self) {
        if let Some(head) = self.head {
            let mut elem = unsafe { Box::from_raw_in(head.as_ptr(), self.alloc.clone()) };

            while let Some(n) = elem.next.take() {
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
    fn get(&mut self) -> Option<HistoryLink<V, A>> {
        match self.head {
            None => None,
            Some(mut elem) => {
                {
                    let elem = unsafe { elem.as_mut() };

                    self.head = elem.next.take();

                    if self.head.is_none() {
                        self.last = None;
                    }
                }

                Some(elem)
            }
        }
    }

    fn put_back(&mut self, chain_start: HistoryLink<V, A>, mut chain_end: HistoryLink<V, A>) {
        match self.last {
            None => {
                self.head = Some(chain_start);
            }
            Some(ref mut last) => {
                unsafe { last.as_mut().next = Some(chain_start) };
            }
        }

        // We need to unlink this, cause it still points to the original history it's been taken
        // from.
        unsafe { chain_end.as_mut().next = None };

        self.last = Some(chain_end);
    }
}

struct HistoryMapState<K, A: Allocator + Clone> {
    current_snapshot_id: CacheSnapshotId,
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
    btree: BTreeMap<K, ElementContainer<V, A>, A>,
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
                updated_elems: StackLinkedList::empty(alloc.clone()),
            },
            reuse: ElementSource::new(alloc),
        }
    }

    pub fn get<'s>(&'s mut self, key: &'s K) -> Option<HistoryMapItemRef<'s, K, V, A>> {
        self.btree
            .get(key)
            .map(|ec| HistoryMapItemRef { key, container: ec })
    }

    pub fn get_mut<'s>(&'s mut self, key: &'s K) -> Option<HistoryMapItemRefMut<'s, K, V, A>> {
        self.btree.get_mut(key).map(|ec| HistoryMapItemRefMut {
            key,
            container: ec,
            cache_state: &mut self.state,
            source: &mut self.reuse,
        })
    }

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
                vacant_entry.insert(ElementContainer::new(
                    v,
                    &mut self.reuse,
                    self.state.alloc.clone(),
                ))
            }
        };

        Ok(HistoryMapItemRefMut {
            key,
            container: v,
            cache_state: &mut self.state,
            source: &mut self.reuse,
        })
    }

    pub fn snapshot(&mut self) -> CacheSnapshotId {
        self.state.current_snapshot_id.increment();
        self.state.current_snapshot_id
    }

    /// Rollbacks the data to the state before the provided `snapshot_id`.
    pub fn rollback(&mut self, snapshot_id: CacheSnapshotId) {
        if snapshot_id == CacheSnapshotId(0) {
            panic!("Rolling to 0'th snapshot is illegal and will cause UB.")
        }

        let mut node = self.state.updated_elems.pop();
        loop {
            match node {
                None => break,
                Some((key, update_snapshot_id)) => {
                    // The items in the address_snapshot_updates are ordered chronologically.
                    if update_snapshot_id < snapshot_id {
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
        let mut node = self.state.updated_elems.peek();
        loop {
            match node {
                None => break,
                Some(ref n) => {
                    let (key, _update_snapshot_id) = &n.value;

                    let item = self
                        .btree
                        .get_mut(key)
                        .expect("We've updated this, so it must be present.");

                    item.commit(&mut self.reuse);

                    node = &n.next;
                }
            }
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

    // TODO used to cleanup in storage and transient storage
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
                container: v,
                cache_state: &mut self.state,
                source: &mut self.reuse,
            })?
        }

        Ok(())
    }

    // TODO only for new preimages publication storage
    pub fn iter(&self) -> impl Iterator<Item = HistoryMapItemRef<'_, K, V, A>> + Clone {
        self.btree.iter().map(|(k, v)| HistoryMapItemRef {
            key: k,
            container: v,
        })
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
                container: self
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
