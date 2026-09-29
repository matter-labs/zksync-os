use super::*;

/// What a flat key of the batch is: the key of an opened leaf, or of a new write.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(super) enum KeyTarget {
    /// The leaf opened at this position
    Opened(usize),
    /// The new write with this ordinal, appended at the saved next free slot plus the ordinal
    Inserted(usize),
}

impl KeyTarget {
    /// The index of the key's leaf.
    #[inline(always)]
    pub(super) fn index<const N: usize, A: Allocator + Clone>(
        self,
        opened: &ChunkedVec<OpenedLeaf<N>, LEAF_CHUNK_LEN, A>,
        new_writes: &[(u64, FlatStorageLeaf<N>)],
    ) -> u64 {
        match self {
            Self::Opened(pos) => opened.get(pos).index,
            Self::Inserted(ordinal) => new_writes[ordinal].0,
        }
    }

    /// The position of the leaf opened at the index of the key's leaf, if any.
    #[inline(always)]
    pub(super) fn resolve<const N: usize, A: Allocator + Clone>(
        self,
        by_index: &BTreeMap<u64, usize, A>,
        opened: &ChunkedVec<OpenedLeaf<N>, LEAF_CHUNK_LEN, A>,
        new_writes: &[(u64, FlatStorageLeaf<N>)],
    ) -> Option<usize> {
        match self {
            Self::Opened(pos) => {
                // an opened leaf is in `by_index` under its own index
                debug_assert_eq!(by_index.get(&opened.get(pos).index), Some(&pos));
                Some(pos)
            }
            Self::Inserted(ordinal) => {
                // The index bounds checks keep opened leaves below the saved next free slot.
                // Keep the lookup for the unhardened differential test against the old code.
                let pos = by_index.get(&new_writes[ordinal].0).copied();
                #[cfg(test)]
                if pos.is_some() {
                    test::count(test::Counter::NewWriteAliasResolved);
                }
                pos
            }
        }
    }
}

/// A key and its target.
pub(super) struct KeySlot {
    pub(super) key: Bytes32,
    pub(super) target: KeyTarget,
}

/// Where the keys with an 8-byte prefix are.
#[derive(Clone, Copy)]
pub(super) enum Head {
    /// The only key with the prefix, in this slot
    One(usize),
    /// Keys that share the prefix, in `overflow`
    Many,
}

/// A key of the overflow map, ordered as a `Bytes32`. The unit tests count its comparisons.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct OverflowKey(Bytes32);

impl Ord for OverflowKey {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        #[cfg(test)]
        test::count(test::Counter::OverflowKeyCmp);
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for OverflowKey {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The flat keys of a batch and their targets: a `BTreeMap<Bytes32, KeyTarget>` with the operations
/// the batch needs, inserting a new key, lookup, length and iteration in key order.
///
/// A key is found by its prefix, its first 8 bytes read big-endian: comparing the prefixes, then the
/// keys on a tie, orders keys as `Bytes32` does. `heads` maps a prefix that only one key has to that
/// key's slot. Keys that share their prefix with another key are in `overflow`, a map of whole keys, so
/// that every operation is O(log n) for any keys, including the neighbour keys an oracle claims.
pub(super) struct KeyIndex<A: Allocator + Clone> {
    pub(super) heads: BTreeMap<u64, Head, A>,
    pub(super) overflow: BTreeMap<OverflowKey, usize, A>,
    pub(super) slots: ChunkedVec<KeySlot, KEY_CHUNK_LEN, A>,
}

impl<A: Allocator + Clone> KeyIndex<A> {
    pub(super) fn with_max_len_in(max_len: usize, allocator: A) -> Self {
        Self {
            heads: BTreeMap::new_in(allocator.clone()),
            overflow: BTreeMap::new_in(allocator.clone()),
            slots: ChunkedVec::with_max_len_in(max_len, allocator),
        }
    }

    /// The number of keys.
    #[inline(always)]
    pub(super) fn len(&self) -> usize {
        self.slots.len()
    }

    pub(super) fn get(&self, key: &Bytes32) -> Option<KeyTarget> {
        match *self.heads.get(&key_prefix(key))? {
            Head::One(slot) => {
                let slot = self.slots.get(slot);
                (slot.key == *key).then_some(slot.target)
            }
            Head::Many => self
                .overflow
                .get(&OverflowKey(*key))
                .map(|&slot| self.slots.get(slot).target),
        }
    }

    /// Inserts a key that is not in the map yet, and panics if it is, as
    /// `assert!(map.insert(key, target).is_none())` does.
    pub(super) fn insert_new(&mut self, key: Bytes32, target: KeyTarget) {
        let slot = self.slots.len();
        match self.heads.entry(key_prefix(&key)) {
            btree_map::Entry::Vacant(entry) => {
                entry.insert(Head::One(slot));
            }
            btree_map::Entry::Occupied(mut entry) => match *entry.get() {
                Head::One(other) => {
                    let other_key = self.slots.get(other).key;
                    assert!(other_key != key, "flat key is already in the batch");
                    // no key in `overflow` has the prefix yet
                    let existing = self.overflow.insert(OverflowKey(other_key), other);
                    debug_assert!(existing.is_none());
                    let existing = self.overflow.insert(OverflowKey(key), slot);
                    debug_assert!(existing.is_none());
                    *entry.get_mut() = Head::Many;
                }
                Head::Many => match self.overflow.entry(OverflowKey(key)) {
                    btree_map::Entry::Vacant(entry) => {
                        entry.insert(slot);
                    }
                    btree_map::Entry::Occupied(_) => panic!("flat key is already in the batch"),
                },
            },
        }
        self.slots.push(KeySlot { key, target });
    }

    /// The keys and their targets, in key order.
    pub(super) fn iter(&self) -> KeyIter<'_, A> {
        KeyIter {
            heads: self.heads.iter(),
            overflow: self.overflow.iter().peekable(),
            many: None,
            slots: &self.slots,
        }
    }
}

/// Iterator over a `KeyIndex` in key order: the heads in the order of their prefixes, where the keys
/// of a `Many` head are the next ones of `overflow`, which is ordered the same way.
pub(super) struct KeyIter<'a, A: Allocator + Clone> {
    pub(super) heads: btree_map::Iter<'a, u64, Head>,
    pub(super) overflow: core::iter::Peekable<btree_map::Iter<'a, OverflowKey, usize>>,
    /// The prefix of the `Many` head whose keys are being taken
    pub(super) many: Option<u64>,
    pub(super) slots: &'a ChunkedVec<KeySlot, KEY_CHUNK_LEN, A>,
}

impl<'a, A: Allocator + Clone> Iterator for KeyIter<'a, A> {
    type Item = (&'a Bytes32, KeyTarget);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(prefix) = self.many {
                if let Some((_, &slot)) = self
                    .overflow
                    .next_if(|(key, _)| key_prefix(&key.0) == prefix)
                {
                    let slot = self.slots.get(slot);
                    return Some((&slot.key, slot.target));
                }
                self.many = None;
            }
            match self.heads.next()? {
                (_, &Head::One(slot)) => {
                    let slot = self.slots.get(slot);
                    return Some((&slot.key, slot.target));
                }
                (&prefix, Head::Many) => self.many = Some(prefix),
            }
        }
    }
}

/// The first 8 bytes of a key, read big-endian.
#[inline(always)]
pub(super) fn key_prefix(key: &Bytes32) -> u64 {
    #[cfg(target_arch = "riscv32")]
    {
        // the first two words, in memory order
        let words = key.as_usize_array_ref();
        prefix_of_words(words[0] as u32, words[1] as u32)
    }
    #[cfg(not(target_arch = "riscv32"))]
    {
        let &[b0, b1, b2, b3, b4, b5, b6, b7, ..] = key.as_u8_array_ref();
        u64::from_be_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
    }
}

/// The big-endian value of the bytes of two little-endian words: each is byte-reversed with `rev8`
/// on riscv32, where `u64::from_be_bytes` is a byte-shuffling sequence (Zbb is not enabled).
#[cfg(any(target_arch = "riscv32", test))]
#[inline(always)]
pub(super) fn prefix_of_words(first: u32, second: u32) -> u64 {
    (u64::from(bswap32(first)) << 32) | u64::from(bswap32(second))
}
