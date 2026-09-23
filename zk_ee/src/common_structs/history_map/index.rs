//! Lookup structures of the history map: they map a key to the handle of its
//! element. The map owns the elements (in its arena) and only asks an index
//! where an element is, so the way keys are searched is a choice of the user:
//! an ordered tree for keys that need range queries, a direct table for keys
//! that are dense small integers, an open-addressing hash table otherwise.
//!
//! None of the fixed-capacity indexes ever reallocates: the proving-mode
//! allocator forbids `grow`, so their storage is allocated once, at full size.

use crate::internal_error;
use crate::system::errors::internal::InternalError;
use crate::utils::word_hasher::BuildWordHasher;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::alloc::Allocator;
use hashbrown::HashMap;

/// Key -> element handle lookup of a [`super::HistoryMap`]. `H` is the opaque
/// handle of an element, a `Copy` value the map hands out and understands.
pub trait ElementIndex<K, H: Copy> {
    /// Number of indexed elements
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Handle of the element with this key, if any
    fn get(&self, key: &K) -> Option<H>;

    /// Indexes a new element. The key must not be in the index yet. Fails when
    /// the index has no room for the key (fixed-capacity indexes).
    fn insert(&mut self, key: K, handle: H) -> Result<(), InternalError>;

    /// Forgets every element
    fn clear(&mut self);

    /// Every indexed handle, in the index's own order. The order is a stable
    /// function of the sequence of insertions, so it is the same in every
    /// execution of the same block.
    fn iter(&self) -> impl ExactSizeIterator<Item = H> + Clone + '_;
}

/// Ordered index: iteration is in key order and ranges of keys can be walked.
pub struct BTreeIndex<K, H, A: Allocator + Clone> {
    pub(super) tree: BTreeMap<K, H, A>,
}

impl<K: Ord, H, A: Allocator + Clone> BTreeIndex<K, H, A> {
    pub fn new_in(alloc: A) -> Self {
        Self {
            tree: BTreeMap::new_in(alloc),
        }
    }
}

impl<K: Ord, H: Copy, A: Allocator + Clone> ElementIndex<K, H> for BTreeIndex<K, H, A> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.tree.len()
    }

    #[inline(always)]
    fn get(&self, key: &K) -> Option<H> {
        self.tree.get(key).copied()
    }

    #[inline(always)]
    fn insert(&mut self, key: K, handle: H) -> Result<(), InternalError> {
        let previous = self.tree.insert(key, handle);
        debug_assert!(previous.is_none(), "key is already indexed");
        Ok(())
    }

    fn clear(&mut self) {
        self.tree.clear();
    }

    fn iter(&self) -> impl ExactSizeIterator<Item = H> + Clone + '_ {
        self.tree.values().copied()
    }
}

/// Direct table for keys that are dense indices below a fixed bound: the key
/// is the position of the handle in the table, so a lookup is one load. Made
/// for keys produced by an interner, which hands them out in order: elements
/// are then inserted in key order too and every insert is a push. Iteration
/// is in key order, which for interned keys is the order of first appearance.
///
/// The table reserves room for every possible key once, at creation, and
/// never reallocates. A key past the end that skips some keys (possible only
/// if the interner saw those keys on a path that does not insert here) is
/// reached by pushing empty slots up to it.
pub struct DenseIndex<H, A: Allocator> {
    /// Slots for the keys `0..slots.len()`, pushed as keys arrive
    slots: Vec<Option<H>, A>,
    /// Number of occupied slots
    len: usize,
}

impl<H: Copy, A: Allocator> DenseIndex<H, A> {
    /// A table for keys in `0..capacity`
    pub fn new_in(capacity: usize, alloc: A) -> Self {
        Self {
            slots: Vec::with_capacity_in(capacity, alloc),
            len: 0,
        }
    }

    /// Number of keys the table has room for
    pub fn capacity(&self) -> usize {
        self.slots.capacity()
    }
}

impl<H: Copy, A: Allocator> ElementIndex<u32, H> for DenseIndex<H, A> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    fn get(&self, key: &u32) -> Option<H> {
        // keys beyond the pushed ones, in or out of range, are simply not there
        self.slots.get(*key as usize).copied().flatten()
    }

    #[inline(always)]
    fn insert(&mut self, key: u32, handle: H) -> Result<(), InternalError> {
        let key = key as usize;
        if key >= self.slots.capacity() {
            return Err(internal_error!("dense index: key is out of range"));
        }
        if key == self.slots.len() {
            // the common case: keys arrive in order
            self.slots.push(Some(handle));
        } else if key > self.slots.len() {
            // within the reserved capacity: no reallocation
            while self.slots.len() < key {
                self.slots.push(None);
            }
            self.slots.push(Some(handle));
        } else {
            let slot = &mut self.slots[key];
            debug_assert!(slot.is_none(), "key is already indexed");
            *slot = Some(handle);
        }
        self.len += 1;
        Ok(())
    }

    fn clear(&mut self) {
        self.slots.clear();
        self.len = 0;
    }

    fn iter(&self) -> impl ExactSizeIterator<Item = H> + Clone + '_ {
        SparseIter {
            slots: self.slots.iter(),
            remaining: self.len,
            handle_of: |handle: &H| *handle,
        }
    }
}

/// Iterator over the occupied slots of a table, with an exact length. `F`
/// extracts the handle from a slot's payload.
#[derive(Clone)]
struct SparseIter<'a, T, F> {
    slots: core::slice::Iter<'a, Option<T>>,
    remaining: usize,
    handle_of: F,
}

impl<'a, T, H: Copy, F: Fn(&T) -> H> Iterator for SparseIter<'a, T, F> {
    type Item = H;

    #[inline]
    fn next(&mut self) -> Option<H> {
        if self.remaining == 0 {
            return None;
        }
        let payload = self.slots.by_ref().flatten().next()?;
        self.remaining -= 1;
        Some((self.handle_of)(payload))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a, T, H: Copy, F: Fn(&T) -> H> ExactSizeIterator for SparseIter<'a, T, F> {}

/// Hash table index for `u32` keys, sized once for a fixed number of keys so
/// it never reallocates. Iteration is in the table's own order.
pub struct HashIndex<H, A: Allocator> {
    map: HashMap<u32, H, BuildWordHasher, A>,
    max_entries: usize,
}

impl<H: Copy, A: Allocator> HashIndex<H, A> {
    /// A table with room for `max_entries` keys, allocated up front
    pub fn new_in(max_entries: usize, alloc: A) -> Self {
        Self {
            map: HashMap::with_capacity_and_hasher_in(max_entries, BuildWordHasher, alloc),
            max_entries,
        }
    }
}

impl<H: Copy, A: Allocator> ElementIndex<u32, H> for HashIndex<H, A> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.map.len()
    }

    #[inline(always)]
    fn get(&self, key: &u32) -> Option<H> {
        self.map.get(key).copied()
    }

    #[inline(always)]
    fn insert(&mut self, key: u32, handle: H) -> Result<(), InternalError> {
        // within the reserved capacity: the table never grows
        if self.map.len() >= self.max_entries {
            return Err(internal_error!("hash index: too many elements"));
        }
        let previous = self.map.insert(key, handle);
        debug_assert!(previous.is_none(), "key is already indexed");
        Ok(())
    }

    fn clear(&mut self) {
        self.map.clear();
    }

    fn iter(&self) -> impl ExactSizeIterator<Item = H> + Clone + '_ {
        self.map.values().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Global;

    #[test]
    fn dense_index_round_trips_and_iterates_in_key_order() {
        let mut index = DenseIndex::<u16, Global>::new_in(8, Global);
        assert!(index.get(&3).is_none());
        index.insert(0, 0).unwrap();
        index.insert(1, 10).unwrap();
        index.insert(3, 30).unwrap();
        index.insert(7, 70).unwrap();
        assert_eq!(index.get(&0), Some(0));
        assert_eq!(index.get(&2), None, "a skipped key stays absent");
        index.insert(2, 20).unwrap();
        assert_eq!(index.get(&2), Some(20));
        assert_eq!(index.len(), 5);
        assert_eq!(index.get(&3), Some(30));
        assert_eq!(index.get(&1), Some(10));
        assert_eq!(index.get(&4), None);
        assert_eq!(index.get(&100), None, "out of range keys are absent");
        assert!(
            index.insert(8, 80).is_err(),
            "out of range keys are refused"
        );
        assert_eq!(index.capacity(), 8, "the reserved room never changes");
        let iter = index.iter();
        assert_eq!(iter.len(), 5);
        assert_eq!(iter.collect::<Vec<_>>(), vec![0, 10, 20, 30, 70]);
        index.clear();
        assert_eq!(index.len(), 0);
        assert!(index.get(&3).is_none());
        assert_eq!(index.iter().len(), 0);
    }

    #[test]
    fn hash_index_round_trips_and_refuses_overflow() {
        let mut index = HashIndex::<u64, Global>::new_in(3, Global);
        for k in [5u32, 9, 13] {
            index.insert(k, k as u64 * 100).unwrap();
        }
        assert_eq!(index.len(), 3);
        for k in [5u32, 9, 13] {
            assert_eq!(index.get(&k), Some(k as u64 * 100));
        }
        assert_eq!(index.get(&17), None);
        assert!(index.insert(17, 1700).is_err(), "over capacity");
        let mut seen: Vec<_> = index.iter().collect();
        assert_eq!(index.iter().len(), 3);
        seen.sort();
        assert_eq!(seen, vec![500, 900, 1300]);
        index.clear();
        assert_eq!(index.get(&5), None);
        index.insert(5, 1).unwrap();
        assert_eq!(index.get(&5), Some(1));
    }

    #[test]
    fn hash_index_spreads_interned_style_keys() {
        // 2^16 keys composed of two dense 8-bit halves
        let mut index = HashIndex::<u32, Global>::new_in(1 << 16, Global);
        let allocated = index.map.capacity();
        for a in 0..256u32 {
            for s in 0..256u32 {
                index.insert((a << 16) | s, a ^ s).unwrap();
            }
        }
        for a in 0..256u32 {
            for s in 0..256u32 {
                assert_eq!(index.get(&((a << 16) | s)), Some(a ^ s));
            }
        }
        assert_eq!(index.get(&(1 << 24)), None);
        assert_eq!(index.map.capacity(), allocated, "the table never grew");
    }
}
