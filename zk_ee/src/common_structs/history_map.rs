//! Contains a key-value map that allows reverting items state.
use alloc::boxed::Box;

use crate::{
    common_structs::{WarmStorageKey, WarmStorageValue},
    system::errors::{InternalError, SystemError},
    utils::stack_linked_list::StackLinkedList,
};
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

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct TransactionId(pub u64);

#[derive(Default, Copy, Clone, Eq, PartialEq, Debug)]
pub enum Appearance {
    #[default]
    Unset,
    Retrieved,
    Updated,
    Deconstructed,
}

#[derive(Clone, Default)]
/// A snapshot. User facing struct.
pub struct CacheSnapshot<V, M> {
    pub appearance: Appearance,
    pub value: V,
    pub metadata: M,
}

type HistoryLink<V, M, A> = NonNull<HistoryItem<V, M, A>>;

/// The history linked list. Always has at least one item with the snapshot id of 0.
struct HistoryItem<V, M, A: Allocator> {
    touch_ss_id: CacheSnapshotId,
    update_tx_id: TransactionId,
    value: CacheSnapshot<V, M>,
    next: Option<HistoryLink<V, M, A>>,
    phantom: PhantomData<(V, M, A)>,
}

pub(crate) struct ElementContainer<V, M, A: Allocator + Clone> {
    ultimate: HistoryLink<V, M, A>,
    penultimate: HistoryLink<V, M, A>,
    head: HistoryLink<V, M, A>,
    alloc: A,
}

pub struct HistoryMapItemRef<'a, K: Clone, V, M, A: Allocator + Clone> {
    key: &'a K,
    container: &'a ElementContainer<V, M, A>,
}

/// Returned to the user to manipulate and access the history.
pub struct HistoryMapItemRefMut<'a, K: Clone, V, M, A: Allocator + Clone> {
    container: &'a mut ElementContainer<V, M, A>,
    cache_state: &'a mut HistoryMapState<K, A>,
    source: &'a mut ElementSource<V, M, A>,
    key: &'a K,
}

impl<V, M, A: Allocator + Clone> Drop for ElementContainer<V, M, A> {
    fn drop(&mut self) {
        let mut elem = unsafe { Box::from_raw_in(self.head.as_ptr(), self.alloc.clone()) };

        while let Some(n) = elem.next.take() {
            let n = unsafe { Box::from_raw_in(n.as_ptr(), self.alloc.clone()) };

            elem = n;
        } // `n` is dropped here.
    } // last elem is dropped here.
}

impl<V, M, A: Allocator> Debug for HistoryItem<V, M, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CacheHistoryItem")
            .field("snapshot_id", &self.touch_ss_id)
            .field("tail", &self.next)
            .finish()
    }
}

impl<T, M, A: Allocator> HistoryItem<T, M, A> {
    pub fn last(&self) -> &CacheSnapshot<T, M> {
        let mut elem = self;

        while let Some(n) = &elem.next {
            elem = unsafe { &*n.as_ptr() };
        }

        &elem.value
    }

    #[allow(dead_code)]
    pub fn diff_operands_total(&self) -> Option<(&CacheSnapshot<T, M>, &CacheSnapshot<T, M>)> {
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

    fn diff_operands_tx(&self) -> Option<(&CacheSnapshot<T, M>, &CacheSnapshot<T, M>)> {
        let right = &self.value;
        let right_tx_id = self.update_tx_id;

        let mut left = None;
        let mut next = &self.next;
        while let Some(n) = &next {
            let n = unsafe { n.as_ref() };
            #[cfg(test)]
            {
                println!(
                    "some item, tx id {} against {} ",
                    n.update_tx_id.0, right_tx_id.0
                )
            }
            if n.update_tx_id != right_tx_id {
                left = Some(n);
                break;
            }

            next = &n.next;
        }

        left.map(|left| (&left.value, right))
    }
}

impl<V, M, A: Allocator + Clone> ElementContainer<V, M, A> {
    #[inline(always)]
    fn new(
        value: V,
        metadata: M,
        appearance: Appearance,
        source: &mut ElementSource<V, M, A>,
        alloc: A,
    ) -> Self {
        let elem = match source.get() {
            Some(mut elem) => {
                {
                    // Safety:
                    let elem = unsafe { elem.as_mut() };

                    // Safety: We *must* rewrite all the links in `elem`.
                    elem.touch_ss_id = CacheSnapshotId(0);
                    elem.update_tx_id = TransactionId(0);
                    elem.value = CacheSnapshot {
                        appearance,
                        value,
                        metadata,
                    };
                    elem.next = None;
                }
                elem
            }
            None => {
                let raw = Box::into_raw(Box::new_in(
                    HistoryItem {
                        touch_ss_id: CacheSnapshotId(0),
                        update_tx_id: TransactionId(0),
                        value: CacheSnapshot {
                            appearance,
                            value,
                            metadata,
                        },
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

    fn rollback(&mut self, reuse: &mut ElementSource<V, M, A>, snapshot_id: CacheSnapshotId) {
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

    fn diff_operands_total(&self) -> Option<(&CacheSnapshot<V, M>, &CacheSnapshot<V, M>)> {
        let entry = unsafe { self.head.as_ref() };
        match entry.next {
            None => None,
            Some(_) => Some((unsafe { &self.ultimate.as_ref().value }, &entry.value)),
        }
    }

    fn diff_operands_tx(&self) -> Option<(&CacheSnapshot<V, M>, &CacheSnapshot<V, M>)> {
        unsafe { self.head.as_ref() }.diff_operands_tx()
    }

    fn commit(&mut self, reuse: &mut ElementSource<V, M, A>) {
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

impl<'a, K, V, M, A> HistoryMapItemRef<'a, K, V, M, A>
where
    K: Clone,
    A: Allocator + Clone,
{
    pub fn key(&self) -> &'a K {
        &self.key
    }

    pub fn current(&self) -> &CacheSnapshot<V, M> {
        unsafe { &self.container.head.as_ref().value }
    }
}

impl<'a, K, V, M, A> HistoryMapItemRefMut<'a, K, V, M, A>
where
    K: Clone + Debug,
    V: Clone,
    M: Clone + Default,
    A: Allocator + Clone,
{
    pub fn current(&self) -> &CacheSnapshot<V, M> {
        unsafe { &self.container.head.as_ref().value }
    }

    pub fn diff_operands_tx(&self) -> Option<(&CacheSnapshot<V, M>, &CacheSnapshot<V, M>)> {
        self.container.diff_operands_tx()
    }

    #[allow(dead_code)]
    pub fn diff_operands_total(&self) -> Option<(&CacheSnapshot<V, M>, &CacheSnapshot<V, M>)> {
        self.container.diff_operands_total()
    }

    #[must_use]
    /// Updates the metadata and retains the appearance.
    pub fn update_metadata<F>(&mut self, f: F) -> Result<(), SystemError>
    where
        F: FnOnce(&mut M) -> Result<(), SystemError>,
    {
        self.update_impl(
            unsafe { self.container.head.as_ref() }.value.appearance,
            |_, m| {
                f(m)?;
                Ok(())
            },
        )
    }

    #[must_use]
    pub fn update<F>(&mut self, f: F) -> Result<(), InternalError>
    where
        F: FnOnce(&mut V, &mut M) -> Result<(), InternalError>,
    {
        let appearance = if self.current().appearance == Appearance::Deconstructed {
            Appearance::Deconstructed
        } else {
            Appearance::Updated
        };
        self.update_impl(appearance, f)
    }

    #[must_use]
    fn update_impl<F, E>(&mut self, set_appearance: Appearance, f: F) -> Result<(), E>
    where
        F: FnOnce(&mut V, &mut M) -> Result<(), E>,
    {
        let history = unsafe { self.container.head.as_mut() };

        if history.touch_ss_id == self.cache_state.current_snapshot_id {
            // We're in the context of the current stapshot.
            f(&mut history.value.value, &mut history.value.metadata)
        } else {
            // The item was last updated before the current snapshot.

            let mut new = match self.source.get() {
                Some(mut elem) => {
                    {
                        let elem = unsafe { elem.as_mut() };

                        elem.value = CacheSnapshot {
                            appearance: set_appearance,
                            value: history.value.value.clone(),
                            metadata: history.value.metadata.clone(),
                        };
                        elem.touch_ss_id = self.cache_state.current_snapshot_id;
                        elem.update_tx_id = self.cache_state.current_transaction_id;
                        elem.next = Some(self.container.head);
                    }

                    elem
                }
                None => {
                    let item = HistoryItem {
                        value: CacheSnapshot {
                            appearance: set_appearance,
                            value: history.value.value.clone(),
                            metadata: history.value.metadata.clone(),
                        },
                        touch_ss_id: self.cache_state.current_snapshot_id,
                        update_tx_id: self.cache_state.current_transaction_id,
                        next: Some(self.container.head),
                        phantom: PhantomData,
                    };
                    let raw = Box::into_raw(Box::new_in(item, self.container.alloc.clone()));
                    unsafe { NonNull::new_unchecked(raw) }
                }
            };

            unsafe {
                let new = new.as_mut();
                f(&mut new.value.value, &mut new.value.metadata)?;
            }

            // let new = Box::into_raw(new);
            // let new = unsafe { NonNull::new_unchecked(new) };

            self.container.head = new;
            if self.container.ultimate == self.container.penultimate {
                // When we have a single item.
                self.container.penultimate = new;
            }

            self.cache_state.updated_elems.push((
                self.key.clone(),
                self.cache_state.current_snapshot_id,
                self.cache_state.current_transaction_id,
            ));

            Ok(())
        }
    }
}

impl<'a, K, V, M, A> HistoryMapItemRefMut<'a, K, V, M, A>
where
    K: Clone + Debug,
    V: Clone + Default,
    M: Clone + Default,
    A: Allocator + Clone,
{
    #[must_use]
    /// Sets appearance to deconstructed. The value itself remains untouched.
    pub fn deconstruct(&mut self) -> Result<(), InternalError> {
        self.update_impl(Appearance::Deconstructed, |_, _m| Ok(()))
    }

    /// Sets appearance to unset. The value itself remains untouched.
    pub fn unset(&mut self) -> Result<(), InternalError> {
        self.update_impl(Appearance::Unset, |_, _m| Ok(()))
    }
}

struct ElementSource<V, M, A: Allocator + Clone> {
    head: Option<HistoryLink<V, M, A>>,
    last: Option<HistoryLink<V, M, A>>,
    alloc: A,
}

impl<V, M, A: Allocator + Clone> Drop for ElementSource<V, M, A> {
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

impl<V, M, A: Allocator + Clone> ElementSource<V, M, A> {
    fn new(alloc: A) -> Self {
        Self {
            head: Default::default(),
            last: Default::default(),
            alloc,
        }
    }
    fn get(&mut self) -> Option<HistoryLink<V, M, A>> {
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

    fn put_back(&mut self, chain_start: HistoryLink<V, M, A>, mut chain_end: HistoryLink<V, M, A>) {
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
    current_transaction_id: TransactionId,
    updated_elems: StackLinkedList<(K, CacheSnapshotId, TransactionId), A>,
    alloc: A,
}

/// A key-value map that allows to store history of the values and to revert their state. The
/// history is a list of stapshots. The snapshots are created on demand between
/// `Self::snapshot(...)` calls.
///
/// Structure:
/// [ keys ] => [ history ] := [ snapshot 0 .. snapshot n ].
pub struct HistoryMap<K, V, M, A: Allocator + Clone> {
    btree: BTreeMap<K, ElementContainer<V, M, A>, A>,
    state: HistoryMapState<K, A>,
    reuse: ElementSource<V, M, A>,
}

impl<K, V, M: Default, A> HistoryMap<K, V, M, A>
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
                current_transaction_id: TransactionId(0),
                updated_elems: StackLinkedList::empty(alloc.clone()),
            },
            reuse: ElementSource::new(alloc),
        }
    }

    pub fn get<'s>(&'s mut self, key: &'s K) -> Option<HistoryMapItemRefMut<'s, K, V, M, A>> {
        self.btree.get_mut(key).map(|ec| HistoryMapItemRefMut {
            key,
            container: ec,
            cache_state: &mut self.state,
            source: &mut self.reuse,
        })
    }

    pub fn get_or_insert<'s, C, E>(
        &'s mut self,
        context: &mut C,
        key: &'s K,
        spawn_v: impl FnOnce(&mut C) -> Result<(V, Appearance), E>,
    ) -> Result<HistoryMapItemRefMut<'s, K, V, M, A>, E> {
        let entry = self.btree.entry(key.clone());

        let v = match entry {
            Entry::Occupied(mut o) => {
                // Safety: Extending lifetime to `self`, no operations on the tree are possible
                // during this item's lifetime.
                unsafe {
                    core::mem::transmute::<
                        &mut ElementContainer<V, M, A>,
                        &'s mut ElementContainer<V, M, A>,
                    >(o.get_mut())
                }
            }
            Entry::Vacant(vacant_entry) => {
                let (v, appearance) = spawn_v(context)?;
                let v = vacant_entry.insert(ElementContainer::new(
                    v,
                    M::default(),
                    appearance,
                    &mut self.reuse,
                    self.state.alloc.clone(),
                ));

                // Safety: Extending lifetime to `self`, no operations on the tree are possible
                // during this item's lifetime.
                unsafe {
                    core::mem::transmute::<
                        &mut ElementContainer<V, M, A>,
                        &'s mut ElementContainer<V, M, A>,
                    >(v)
                }
            }
        };

        Ok(HistoryMapItemRefMut {
            key,
            container: v,
            cache_state: &mut self.state,
            source: &mut self.reuse,
        })
    }

    pub fn snapshot(&mut self, tx_id: TransactionId) -> CacheSnapshotId {
        debug_assert!(self.state.current_transaction_id <= tx_id);

        self.state.current_snapshot_id.increment();
        self.state.current_transaction_id = tx_id;
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
                Some((key, update_snapshot_id, x)) => {
                    // The items in the address_snapshot_updates are ordered chronologically.
                    if update_snapshot_id < snapshot_id {
                        self.state.updated_elems.push((key, update_snapshot_id, x));
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
                    let (key, _update_snapshot_id, _) = &n.value;

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
        F: FnMut(&CacheSnapshot<V, M>, &CacheSnapshot<V, M>, &K) -> Result<(), E>,
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
        F: FnMut(HistoryMapItemRefMut<K, V, M, A>) -> Result<(), InternalError>,
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
    pub fn iter(&self) -> impl Iterator<Item = HistoryMapItemRef<'_, K, V, M, A>> {
        self.btree.iter().map(|(k, v)| HistoryMapItemRef {
            key: k,
            container: v,
        })
    }

    // TODO use only for account cache
    pub fn iter_altered_since_commit(
        &self,
    ) -> impl Iterator<Item = HistoryMapItemRef<'_, K, V, M, A>> {
        self.state
            .updated_elems
            .iter()
            .map(|(k, _, _)| HistoryMapItemRef {
                key: k,
                container: self
                    .btree
                    .get(k)
                    .expect("We've updated this, so it must be present."),
            })
    }
}

impl<A, M> HistoryMap<WarmStorageKey, crate::utils::Bytes32, M, A>
where
    A: Allocator + Clone,
{
    pub fn iter_as_storage_types(
        &self,
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + use<'_, A, M> + Clone {
        self.btree.iter().map(|(k, v)| {
            let history = unsafe { v.head.as_ref() };
            (
                *k,
                // Using the WarmStorageValue temporarily till it's outed from the codebase. We're
                // not actually 'using' it.
                WarmStorageValue {
                    current_value: history.value.value,
                    is_new_storage_slot: history.last().appearance == Appearance::Unset,
                    initial_value: history.last().value,
                    initial_value_used: true,
                    ..Default::default()
                },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::Global;

    use super::{Appearance, HistoryMap};

    #[test]
    fn miri_retrieve_single_elem() {
        let mut map = HistoryMap::<usize, usize, (), Global>::new(Global);

        let v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((1, Appearance::Retrieved)))
            .unwrap();

        assert_eq!(1, v.current().value);
    }

    #[test]
    fn miri_diff_elem_total() {
        let mut map = HistoryMap::<usize, usize, (), Global>::new(Global);

        map.snapshot(super::TransactionId(1));

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((1, Appearance::Retrieved)))
            .unwrap();

        v.update(|x, _| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        let (l, r) = v.diff_operands_total().unwrap();

        assert_eq!(1, l.value);
        assert_eq!(2, r.value);
    }

    #[test]
    fn miri_diff_tree_total() {
        let mut map = HistoryMap::<usize, usize, (), Global>::new(Global);

        map.snapshot(super::TransactionId(1));

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((1, Appearance::Retrieved)))
            .unwrap();

        v.update(|x, _| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
            assert_eq!(1, l.value);
            assert_eq!(2, r.value);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn miri_commit_1() {
        let mut map = HistoryMap::<usize, usize, (), Global>::new(Global);

        map.snapshot(super::TransactionId(1));

        map.get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((1, Appearance::Retrieved)))
            .unwrap();

        map.commit();

        map.for_total_diff_operands::<_, ()>(|_, _, _| {
            panic!("No changes were made.");
        })
        .unwrap();
    }

    #[test]
    fn miri_commit_2() {
        let mut map = HistoryMap::<usize, usize, (), Global>::new(Global);

        map.snapshot(super::TransactionId(1));

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((1, Appearance::Retrieved)))
            .unwrap();

        v.update(|x, _| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        map.commit();

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
            assert_eq!(1, l.value);
            assert_eq!(2, r.value);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn miri_commit_3() {
        let mut map = HistoryMap::<usize, usize, (), Global>::new(Global);

        map.snapshot(super::TransactionId(1));

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((1, Appearance::Retrieved)))
            .unwrap();

        v.update(|x, _| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        map.snapshot(super::TransactionId(1));

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((4, Appearance::Retrieved)))
            .unwrap();

        v.update(|x, _| {
            *x = 3;
            Ok(())
        })
        .unwrap();

        map.commit();

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
            assert_eq!(1, l.value);
            assert_eq!(3, r.value);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn miri_rollback() {
        let mut map = HistoryMap::<usize, usize, (), Global>::new(Global);

        map.snapshot(super::TransactionId(1));

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((1, Appearance::Retrieved)))
            .unwrap();

        v.update(|x, _| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        let ss = map.snapshot(super::TransactionId(1));

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((4, Appearance::Retrieved)))
            .unwrap();

        v.update(|x, _| {
            *x = 3;
            Ok(())
        })
        .unwrap();

        map.snapshot(super::TransactionId(1));

        map.rollback(ss);

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
            assert_eq!(1, l.value);
            assert_eq!(2, r.value);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn miri_rollback_reuse() {
        let mut map = HistoryMap::<usize, usize, (), Global>::new(Global);

        map.snapshot(super::TransactionId(1));

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((1, Appearance::Retrieved)))
            .unwrap();

        v.update(|x, _| {
            *x = 2;
            Ok(())
        })
        .unwrap();

        // We'll rollback to this point.
        let ss = map.snapshot(super::TransactionId(1));

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((4, Appearance::Retrieved)))
            .unwrap();

        // This snapshot will be rollbacked.
        v.update(|x, _| {
            *x = 3;
            Ok(())
        })
        .unwrap();

        // Just for fun.
        map.snapshot(super::TransactionId(1));

        map.rollback(ss);

        let mut v = map
            .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((5, Appearance::Retrieved)))
            .unwrap();

        // This will create a new snapshot and will reuse the one that rollbacked.
        v.update(|x, _| {
            *x = 6;
            Ok(())
        })
        .unwrap();

        map.for_total_diff_operands::<_, ()>(|l, r, k| {
            assert_eq!(1, l.value);
            assert_eq!(6, r.value);
            assert_eq!(1, *k);

            Ok(())
        })
        .unwrap();
    }
}
