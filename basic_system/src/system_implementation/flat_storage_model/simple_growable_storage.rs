// We want to implement a flat storage (so address + storage slot under address for a single homogeneous key).
// We should care about potential attacks on the particular path, because one can make access to particular key more expensive
// by accessing specially crafted storage slot index under controlled address to make long common prefix, so we can not just
// use some dynamically-growable tree as it would lead to 160 bits common prefix attacks at complexity 2^80 that is unacceptable.

// So instead we use an implementation of the linked-list backed via fixed length array. The logic is as following:
// - we create empty "array" of size 2^64 (reasonable upper bound on number of storage slots ever), and maintain a counter for "next empty",
// - we also create two formal start/end nodes with min and max "key" values, that is reasonable is "key" is itself an output of "good" hash function
// - array element structure is essentially (key, value) and some other info that we consider needed for pubdata efficiency,
//   plus an index that is a "pointer" to the leaf with the next key in the tree w.r.t. the key order.
// - if we need to insert for particular "key" we treat a key as integer and non-deterministically either
//   - a) claim that such key exists and we expose a merkle path to the leaf with a corresponding "key"
//   - b) claim that such key doesn't exist and we open two leaves `a` and `b` such that `a.next` is the index of `b`, and `a.key < key < b.key`, and so
//      insert an element to the storage
// In such case access complexity for existing keys is just 64 hashes for read/128 for write, and writing "new" is 2 writes + 1 read.
// For a case if we need to prove reading a "fresh" key, but do not need to write, we just need to read 2 array elements, but are not
// mandated to write anything.

use alloc::alloc::Global;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::alloc::Allocator;
use crypto::MiniDigest;
use either::Either;
use zk_ee::common_structs::derive_flat_storage_key_with_hasher;
use zk_ee::common_structs::state_root_view::StateRootView;
use zk_ee::common_structs::{WarmStorageKey, WarmStorageValue};
use zk_ee::internal_error;
use zk_ee::{
    kv_markers::{ExactSizeChain, ExactSizeChainN, UsizeDeserializable, UsizeSerializable},
    memory::stack_trait::Stack,
    system::{errors::internal::InternalError, logger::Logger},
    system_io_oracle::*,
    types_config::EthereumIOTypesConfig,
    utils::Bytes32,
};

// we use u64 types below, but in practice smaller depth will be used

pub const MIN_KEY_LEAF_MARKER_IDX: u64 = 0;
pub const MAX_KEY_LEAF_MARKER_IDX: u64 = 1;

// Note: all zeroes is well-defined for empty array slot, as we will insert two guardian values upon creation
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct FlatStorageLeaf<const N: usize> {
    pub key: Bytes32,
    pub value: Bytes32,
    pub next: u64,
}

impl<const N: usize> UsizeSerializable for FlatStorageLeaf<N> {
    const USIZE_LEN: usize =
        <Bytes32 as UsizeSerializable>::USIZE_LEN * 2 + <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.key),
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.value),
                UsizeSerializable::iter(&self.next),
            ),
        )
    }
}

impl<const N: usize> UsizeDeserializable for FlatStorageLeaf<N> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let key = UsizeDeserializable::from_iter(src)?;
        let value = UsizeDeserializable::from_iter(src)?;
        let next = UsizeDeserializable::from_iter(src)?;

        let new = Self { key, value, next };

        Ok(new)
    }
}

impl<const N: usize> FlatStorageLeaf<N> {
    pub fn empty() -> Self {
        Self {
            key: Bytes32::ZERO,
            value: Bytes32::ZERO,
            next: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self == &Self::empty()
    }

    pub fn update_digest<D: crypto::MiniDigest>(&self, digest: &mut D) {
        digest.update(self.key.as_u8_array_ref());
        digest.update(self.value.as_u8_array_ref());
        digest.update(&self.next.to_le_bytes());
    }
}

pub trait FlatStorageHasher: 'static + Send + Sync + core::fmt::Debug {
    fn new() -> Self;
    fn hash_leaf<const N: usize>(&mut self, leaf: &FlatStorageLeaf<N>) -> Bytes32;
    fn hash_node(&mut self, left_node: &Bytes32, right_node: &Bytes32) -> Bytes32;
}

#[derive(Clone, Debug)]
pub struct Blake2sStorageHasher {
    hasher: crypto::blake2s::Blake2s256,
}

impl FlatStorageHasher for Blake2sStorageHasher {
    fn new() -> Self {
        Self {
            hasher: crypto::blake2s::Blake2s256::new(),
        }
    }
    fn hash_leaf<const N: usize>(&mut self, leaf: &FlatStorageLeaf<N>) -> Bytes32 {
        leaf.update_digest(&mut self.hasher);
        Bytes32::from_array(self.hasher.finalize_reset())
    }

    fn hash_node(&mut self, left_node: &Bytes32, right_node: &Bytes32) -> Bytes32 {
        self.hasher.update(left_node.as_u8_array_ref());
        self.hasher.update(right_node.as_u8_array_ref());
        Bytes32::from_array(self.hasher.finalize_reset())
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct FlatStorageCommitment<const N: usize> {
    pub root: Bytes32,
    pub next_free_slot: u64, // NOTE: this will effectively be our "next enumeration counter" for pubdata purposes
}

impl<const N: usize> UsizeSerializable for FlatStorageCommitment<N> {
    const USIZE_LEN: usize =
        <Bytes32 as UsizeSerializable>::USIZE_LEN + <u64 as UsizeSerializable>::USIZE_LEN;
    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.root),
            UsizeSerializable::iter(&self.next_free_slot),
        )
    }
}

impl<const N: usize> UsizeDeserializable for FlatStorageCommitment<N> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let root = UsizeDeserializable::from_iter(src)?;
        let next_free_slot = UsizeDeserializable::from_iter(src)?;

        let new = Self {
            root,
            next_free_slot,
        };

        Ok(new)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LeafUpdateRecord {
    pub value: Option<Bytes32>,
    pub next: Option<u64>,
}

impl LeafUpdateRecord {
    pub fn set_next(&mut self, value: u64) {
        assert!(self.next.is_none());
        self.next = Some(value);
    }
}

impl<const N: usize> StateRootView<EthereumIOTypesConfig> for FlatStorageCommitment<N> {
    ///
    /// Vefifies all the state reads and applies writes.
    ///
    fn verify_and_apply_batch<'a, O: IOOracle, A: Allocator + Clone + Default>(
        &mut self,
        oracle: &mut O,
        source: impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone,
        allocator: A,
        logger: &mut impl Logger,
    ) -> Result<(), InternalError> {
        // we know that for our storage model we have a luxury of amortizing all new writes,
        // so our strategy is to:
        // - verify all reads
        // - apply non-initial writes
        // - get a proof of the last stored item (by index)
        // - compute leaf hashes for a batch and extend tree
        // - perform updates of pointers in existing section of the tree
        // - carefully handle the case if we potentially have pointers to each other in the appended section

        // and then we can go even further, by checking early intersections, and reducing a number of accesses
        // if we have many of them (by trading control flow vs more hash functions)

        let mut key_to_index_cache = BTreeMap::<Bytes32, u64, A>::new_in(allocator.clone());
        let mut index_to_leaf_cache = BTreeMap::<
            u64,
            (
                LeafProof<N, Blake2sStorageHasher, A>,
                Option<LeafUpdateRecord>,
            ),
            A,
        >::new_in(allocator.clone());

        // If there was no IO, just return
        if source.clone().next().is_none() {
            return Ok(());
        }

        let reads_iter = source
            .clone()
            .filter(|(_, v)| v.current_value == v.initial_value);
        let writes_iter = source
            .clone()
            .filter(|(_, v)| v.current_value != v.initial_value);

        let num_new_writes = source
            .clone()
            .filter(|(_, v)| v.current_value != v.initial_value && v.is_new_storage_slot)
            .count();

        let mut new_writes = Vec::with_capacity_in(num_new_writes, allocator.clone());

        // save it for later
        let saved_next_free_slot = self.next_free_slot;

        let mut num_nonexisting_reads = 0;

        let mut hasher = crypto::blake2s::Blake2s256::new();
        for (key, value) in reads_iter {
            let flat_key = derive_flat_storage_key_with_hasher(&key.address, &key.key, &mut hasher);
            // reads
            let expect_new = value.is_new_storage_slot;
            assert!(value.initial_value_used);
            if expect_new {
                // assert_eq!(
                //     value.initial_value,
                //     WarmStorageValue::TRIVIAL_VALUE,
                //     "initial value of empty slot must be trivial"
                // );
                num_nonexisting_reads += 1;

                // TODO(EVM-1075): debug implementation for B160 uses global alloc, which panics in ZKsync OS
                #[cfg(not(target_arch = "riscv32"))]
                let _ = logger.write_fmt(format_args!(
                    "checking empty read for address = {:?}, key = {:?}\n",
                    &key.address, &key.key,
                ));

                let previous_idx = get_prev_index::<O>(oracle, &flat_key);
                assert!(previous_idx < saved_next_free_slot);
                let next_idx;

                // Check if indexes are in cache,
                // otherwise get leaf and add to caches.
                {
                    let previous_check = |previous: &LeafProof<N, Blake2sStorageHasher, A>| {
                        assert!(previous.leaf.key < flat_key);
                    };
                    match index_to_leaf_cache.get(&previous_idx) {
                        None => {
                            let prev = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(
                                oracle,
                                previous_idx,
                            )
                            .proof
                            .existing;
                            previous_check(&prev);

                            next_idx = prev.leaf.next;
                            assert!(next_idx < saved_next_free_slot);
                            let existing = key_to_index_cache.insert(prev.leaf.key, previous_idx);
                            assert!(existing.is_none());
                            index_to_leaf_cache.insert(previous_idx, (prev, None));
                        }
                        Some((prev, _)) => {
                            previous_check(prev);
                            next_idx = prev.leaf.next;
                        }
                    }
                }

                {
                    let next_check = |next: &LeafProof<N, Blake2sStorageHasher, A>| {
                        assert!(next.leaf.key > flat_key);
                    };
                    match index_to_leaf_cache.get(&next_idx) {
                        None => {
                            let next = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(
                                oracle, next_idx,
                            )
                            .proof
                            .existing;
                            next_check(&next);
                            let existing = key_to_index_cache.insert(next.leaf.key, next_idx);
                            assert!(existing.is_none());
                            index_to_leaf_cache.insert(next_idx, (next, None));
                        }
                        Some((leaf, _)) => {
                            next_check(leaf);
                        }
                    }
                }
                // now we will need to check merkle paths for that indexes
            } else {
                // TODO(EVM-1075): debug implementation for B160 uses global alloc, which panics in ZKsync OS
                #[cfg(not(target_arch = "riscv32"))]
                let _ = logger.write_fmt(format_args!(
                    "checking existing read for address = {:?}, key = {:?}, value = {:?}\n",
                    &key.address, &key.key, value.current_value,
                ));

                let index = get_index::<O>(oracle, &flat_key);

                let check = |leaf: &LeafProof<N, Blake2sStorageHasher, A>| {
                    assert_eq!(leaf.leaf.key, flat_key);
                    assert_eq!(leaf.leaf.value, value.current_value);
                };

                // Check if index is  in cache,
                // otherwise get leaf and add to caches.
                match index_to_leaf_cache.get(&index) {
                    None => {
                        let leaf =
                            get_proof_for_index::<N, O, Blake2sStorageHasher, A>(oracle, index)
                                .proof
                                .existing;
                        check(&leaf);
                        let existing = key_to_index_cache.insert(leaf.leaf.key, index);
                        assert!(existing.is_none());
                        index_to_leaf_cache.insert(index, (leaf, None));
                    }
                    Some((leaf, _)) => {
                        check(leaf);
                    }
                }
            }
        }

        // NOTE: we effectively build a linked list backed by the fixed capacity (merkle tree) array,
        // and our `key_to_index_cache` is just another representation of it

        let mut num_total_writes = 0;

        for (key, value) in writes_iter {
            num_total_writes += 1;
            let flat_key = derive_flat_storage_key_with_hasher(&key.address, &key.key, &mut hasher);
            // writes
            let expect_new = value.is_new_storage_slot;
            if expect_new {
                // TODO(EVM-1075): debug implementation for B160 uses global alloc, which panics in ZKsync OS
                #[cfg(not(target_arch = "riscv32"))]
                let _ = logger.write_fmt(format_args!(
                    "applying initial write for address = {:?}, key = {:?}, value {:?} -> {:?}\n",
                    &key.address, &key.key, &value.initial_value, &value.current_value
                ));

                // since it's new, we always ask for neighbours
                let previous_idx = get_prev_index::<O>(oracle, &flat_key);
                assert!(previous_idx < saved_next_free_slot);
                let next_idx;
                // but we can still have two cases regarding touching such neighbours, if we insert something in the middle

                match index_to_leaf_cache.get(&previous_idx) {
                    Some((previous, _)) => {
                        next_idx = previous.leaf.next;
                        assert!(previous.leaf.key < flat_key);
                    }
                    None => {
                        let previous = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(
                            oracle,
                            previous_idx,
                        );
                        next_idx = previous.proof.existing.leaf.next;
                        assert!(next_idx < saved_next_free_slot);
                        assert!(previous.proof.existing.leaf.key < flat_key);
                        // and we can insert
                        let existing = key_to_index_cache
                            .insert(previous.proof.existing.leaf.key, previous_idx);
                        assert!(existing.is_none());
                        let existing = index_to_leaf_cache.insert(
                            previous_idx,
                            (
                                previous.proof.existing,
                                None, // will be adjusted later one
                            ),
                        );
                        assert!(existing.is_none());
                    }
                }
                // same for next
                match index_to_leaf_cache.get(&next_idx) {
                    Some((next, _)) => {
                        assert!(next.leaf.key > flat_key);
                    }
                    None => {
                        let next =
                            get_proof_for_index::<N, O, Blake2sStorageHasher, A>(oracle, next_idx);
                        assert!(next.proof.existing.leaf.key > flat_key);
                        // and we can insert
                        let existing =
                            key_to_index_cache.insert(next.proof.existing.leaf.key, next_idx);
                        assert!(existing.is_none());
                        let existing = index_to_leaf_cache.insert(
                            next_idx,
                            (
                                next.proof.existing,
                                None, // will be adjusted later one
                            ),
                        );
                        assert!(existing.is_none());
                    }
                }

                // and we will append it to the tree
                let index = self.next_free_slot;
                self.next_free_slot += 1;
                // insert only in one of the caches
                let existing = key_to_index_cache.insert(flat_key, index);
                assert!(existing.is_none());
                new_writes.push((
                    index,
                    FlatStorageLeaf::<N> {
                        key: flat_key,
                        value: value.current_value,
                        next: 0, // will be adjusted later
                    },
                ));
            } else {
                // TODO(EVM-1075): debug implementation for B160 uses global alloc, which panics in ZKsync OS
                #[cfg(not(target_arch = "riscv32"))]
                let _ = logger.write_fmt(format_args!(
                    "applying repeated write for address = {:?}, key = {:?}, value {:?} -> {:?}\n",
                    &key.address, &key.key, &value.initial_value, &value.current_value
                ));
                // here we branch because we COULD have requested this one as a read witness

                if let Some(existing_index) = key_to_index_cache.get(&flat_key).copied() {
                    let (existing_leaf, modification) = index_to_leaf_cache
                        .get_mut(&existing_index)
                        .expect("must be in cache");
                    assert_eq!(existing_leaf.leaf.key, flat_key);
                    assert_eq!(existing_leaf.leaf.value, value.initial_value);
                    assert!(modification.is_none());
                    *modification = Some(LeafUpdateRecord {
                        value: Some(value.current_value),
                        next: None, // this value will be adjusted later
                    });
                } else {
                    let index = get_index::<O>(oracle, &flat_key);
                    let leaf = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(oracle, index);
                    assert_eq!(leaf.proof.existing.leaf.key, flat_key);
                    assert_eq!(leaf.proof.existing.leaf.value, value.initial_value);
                    let existing = key_to_index_cache.insert(leaf.proof.existing.leaf.key, index);
                    assert!(existing.is_none());
                    let existing = index_to_leaf_cache.insert(
                        index,
                        (
                            leaf.proof.existing,
                            Some(LeafUpdateRecord {
                                value: Some(value.current_value),
                                next: None, // this value will be adjusted later
                            }),
                        ),
                    );
                    assert!(existing.is_none());
                }
            }
        }

        // now we can adjust indexes

        if num_nonexisting_reads + num_new_writes == 0 {
            // it means that we do not need to remake a linked list and just read slots with existing matching keys or update them
        } else {
            // It should be a case when we have at least one initial write or empty read - we should at least have key::MIN and key::MAX,
            // so we can take
            let num_elements = key_to_index_cache.len();
            assert!(
                num_elements >= 2,
                "There should be at least bound leaves in the cache"
            );

            // here we should collect keys and walk over tuples of them
            // first one is special, and last one is special
            let mut keys = key_to_index_cache.keys();
            let first = keys.next().unwrap();
            let first_index = key_to_index_cache[first];
            let second = keys.next().unwrap();
            let second_index = key_to_index_cache[second];

            // it's initial guard leaf with key == 0
            let (leaf, modification) = index_to_leaf_cache
                .get_mut(&first_index)
                .expect("first leaf");
            if first_index == MIN_KEY_LEAF_MARKER_IDX {
                assert!(modification.is_none());
                assert_eq!(leaf.leaf.key, Bytes32::ZERO);
            }

            if second_index >= saved_next_free_slot {
                // We only need to relink pointers if either the current leaf is new, or the previously existing leaf
                // has a new leaf inserted after it, or both. If both the current and next leaves are old, linking them is generally incorrect.
                // As an example consider a leaf with `key_1` loaded as a next leaf for one or more inserts, and `key_2` loaded as a previous leaf
                // for one or more inserts, such that `key_1 < key_2` and there are no other leaves in `key_1..key_2` loaded for the proof. In this case,
                // unless we do filtering above, we'll link these 2 leaves together, but there may be an indefinite number of keys in the `key_1..key_2` range in the full tree!
                if let Some(modification) = modification.as_mut() {
                    modification.set_next(second_index);
                } else {
                    // no value update, but only index update
                    *modification = Some(LeafUpdateRecord {
                        value: None,
                        next: Some(second_index),
                    });
                }
            }

            // now we work over sets of 3
            let mut current_index = second_index;
            for next in keys {
                let next_index = key_to_index_cache[next];
                // we need to modify current
                match index_to_leaf_cache.get_mut(&current_index) {
                    Some((leaf, modification)) => {
                        // See the explanation above why this check is required.
                        if next_index >= saved_next_free_slot {
                            if let Some(modification) = modification.as_mut() {
                                if leaf.leaf.next != next_index {
                                    modification.set_next(next_index);
                                }
                            } else {
                                let mut new_modification = LeafUpdateRecord {
                                    value: None,
                                    next: None,
                                };
                                if leaf.leaf.next != next_index {
                                    new_modification.set_next(next_index);
                                }
                                *modification = Some(new_modification);
                            }
                        }
                    }
                    None => {
                        // it's initial write
                        let idx = current_index - saved_next_free_slot;
                        let (expected_tree_index, leaf) = &mut new_writes[idx as usize];
                        assert_eq!(*expected_tree_index, current_index);
                        leaf.next = next_index;
                    }
                }

                current_index = next_index;
            }

            // and finish with last two

            let last_index = current_index;

            let (leaf, modification) = index_to_leaf_cache.get(&last_index).expect("last leaf");
            if last_index == MAX_KEY_LEAF_MARKER_IDX {
                assert!(modification.is_none());
                assert_eq!(leaf.leaf.key, Bytes32::MAX);
            }

            // now we potentially augment `index_to_leaf_cache` using the path for the "rightmost" tree element,
            // and recompute a binary tree by folding either computed nodes, or computed + witness

            if num_new_writes > 0 {
                // get rightmost path as it'll be needed anyway to append more leaves
                if index_to_leaf_cache.contains_key(&(saved_next_free_slot - 1)) == false {
                    let proof = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(
                        oracle,
                        saved_next_free_slot - 1,
                    );
                    index_to_leaf_cache
                        .insert(saved_next_free_slot - 1, (proof.proof.existing, None));
                }
            }
        }

        // and now we join
        let mut hasher = Blake2sStorageHasher::new();
        let empty_hashes =
            compute_empty_hashes::<N, Blake2sStorageHasher, A>(&mut hasher, allocator.clone());

        // now we should have fun and join the paths
        let buffer_size = index_to_leaf_cache.len() + num_new_writes;
        let mut current_hashes_buffer = Vec::with_capacity_in(buffer_size, allocator.clone());
        let mut next_hashes_buffer = Vec::with_capacity_in(buffer_size, allocator.clone());

        for (index, (leaf, modification)) in index_to_leaf_cache.iter() {
            let leaf_hash = hasher.hash_leaf(&leaf.leaf);
            let updated_hash = if let Some(modification) = modification.as_ref() {
                let mut updated_leaf = leaf.leaf;
                if let Some(value) = modification.value.as_ref() {
                    updated_leaf.value = *value;
                }
                if let Some(next) = modification.next.as_ref() {
                    updated_leaf.next = *next;
                }
                let updated_leaf_hash = hasher.hash_leaf(&updated_leaf);

                Some(updated_leaf_hash)
            } else {
                None
            };

            current_hashes_buffer.push((*index, *index, Some(leaf_hash), updated_hash));
        }
        // append new writes
        for (index, new_leaf) in new_writes.into_iter() {
            let leaf_hash = hasher.hash_leaf(&new_leaf);
            current_hashes_buffer.push((index, index, None, Some(leaf_hash)));
        }

        // then merge
        fn can_merge(pair: (u64, u64)) -> bool {
            let (a, b) = pair;
            debug_assert_ne!(a, b);
            a & !1 == b & !1
        }

        let process_single = |a: &(u64, u64, Option<Bytes32>, Option<Bytes32>),
                              level: u32,
                              dst: &mut Vec<(u64, u64, Option<Bytes32>, Option<Bytes32>), A>,
                              hasher: &mut Blake2sStorageHasher| {
            let is_left = a.0 & 1 == 0;
            let proof = match index_to_leaf_cache.get(&a.1) {
                Some((leaf, _)) => &leaf.path[level as usize],
                None => {
                    // use default
                    &empty_hashes[level as usize]
                }
            };

            let read_path = if let Some(read_path) = a.2.as_ref() {
                let (left, right) = if is_left {
                    (read_path, proof)
                } else {
                    (proof, read_path)
                };
                let node_hash = hasher.hash_node(left, right);

                Some(node_hash)
            } else {
                None
            };

            let write_path = if let Some(write_path) = a.3.as_ref() {
                let (left, right) = if is_left {
                    (write_path, proof)
                } else {
                    (proof, write_path)
                };
                let node_hash = hasher.hash_node(left, right);

                Some(node_hash)
            } else {
                None
            };

            let index = a.0 >> 1;
            dst.push((index, a.1, read_path, write_path));
        };

        for level in 0..N {
            assert!(!current_hashes_buffer.is_empty());
            if current_hashes_buffer.len() == 1 {
                // just progress
                let a = &current_hashes_buffer[0];
                process_single(a, level as u32, &mut next_hashes_buffer, &mut hasher);

                current_hashes_buffer.clear();
                core::mem::swap(&mut current_hashes_buffer, &mut next_hashes_buffer);
                continue;
            }

            let mut next_merged = false;
            let num_windows = current_hashes_buffer.len() - 1;
            let mut last_merged = false;
            for (idx, [a, b]) in current_hashes_buffer.array_windows::<2>().enumerate() {
                if next_merged {
                    next_merged = false;
                    continue;
                }

                assert_ne!(a.0, b.0);

                let is_last = idx == num_windows - 1;
                let empty_path = &empty_hashes[level];

                if can_merge((a.0, b.0)) {
                    // two paths will converge now
                    let (read_hash, a_read_path, b_read_path) = match (a.2.as_ref(), b.2.as_ref()) {
                        (Some(a_read_path), Some(b_read_path)) => {
                            let node_hash = hasher.hash_node(a_read_path, b_read_path);
                            (Some(node_hash), a_read_path, b_read_path)
                        }
                        (Some(a_read_path), None) => {
                            let node_hash = hasher.hash_node(a_read_path, empty_path);
                            (Some(node_hash), a_read_path, empty_path)
                        }
                        (None, Some(b_read_path)) => {
                            let node_hash = hasher.hash_node(empty_path, b_read_path);
                            (Some(node_hash), empty_path, b_read_path)
                        }
                        (None, None) => {
                            assert!(a.1 >= saved_next_free_slot);
                            assert!(b.1 >= saved_next_free_slot);
                            (None, empty_path, empty_path)
                        }
                    };

                    let write_hash = match (a.3.as_ref(), b.3.as_ref()) {
                        (Some(a_write_path), Some(b_write_path)) => {
                            let node_hash = hasher.hash_node(a_write_path, b_write_path);
                            Some(node_hash)
                        }
                        (Some(a_write_path), None) => {
                            let node_hash = hasher.hash_node(a_write_path, b_read_path);
                            Some(node_hash)
                        }
                        (None, Some(b_write_path)) => {
                            let node_hash = hasher.hash_node(a_read_path, b_write_path);
                            Some(node_hash)
                        }
                        (None, None) => {
                            assert!(read_hash.is_some());
                            None
                        }
                    };

                    let merged_index = a.0 >> 1;
                    debug_assert_eq!(merged_index, b.0 >> 1);
                    next_hashes_buffer.push((merged_index, a.1, read_hash, write_hash));
                    next_merged = true;
                    if is_last {
                        last_merged = true;
                    }
                } else {
                    // progress for `a`` only
                    process_single(a, level as u32, &mut next_hashes_buffer, &mut hasher);
                }
            }

            if last_merged == false {
                // we need to progress last
                let a: &(u64, u64, Option<Bytes32>, Option<Bytes32>) =
                    current_hashes_buffer.last().unwrap();
                process_single(a, level as u32, &mut next_hashes_buffer, &mut hasher);
            }

            current_hashes_buffer.clear();
            core::mem::swap(&mut current_hashes_buffer, &mut next_hashes_buffer);
        }

        assert!(next_hashes_buffer.is_empty());
        assert_eq!(current_hashes_buffer.len(), 1);
        assert_eq!(
            current_hashes_buffer[0].2.expect("read root"),
            self.root,
            "storage reads are inconsistent"
        );
        // if we have new root - use it
        if let Some(new_root) = current_hashes_buffer[0].3 {
            if num_total_writes > 0 {
                assert!(
                    new_root != self.root,
                    "hash collision on state root with non-zero number of writes"
                );
            }
            self.root = new_root;
        } else {
            assert_eq!(num_new_writes, 0);
            // root should not change in such case
        }

        Ok(())
    }
}

fn get_prev_index<O: IOOracle>(oracle: &mut O, flat_key: &Bytes32) -> u64 {
    let mut it = oracle
        .create_oracle_access_iterator::<PrevIndexIterator>(*flat_key)
        .expect("must get iterator for prev index");
    UsizeDeserializable::from_iter(&mut it).expect("must deserialize prev index")
}

fn get_index<O: IOOracle>(oracle: &mut O, flat_key: &Bytes32) -> u64 {
    let mut it = oracle
        .create_oracle_access_iterator::<ExactIndexIterator>(*flat_key)
        .expect("must get iterator for neighbours");
    let index: u64 = UsizeDeserializable::from_iter(&mut it).expect("must deserialize neighbours");

    index
}

fn get_proof_for_index<
    const N: usize,
    O: IOOracle,
    H: FlatStorageHasher,
    A: Allocator + Clone + Default,
>(
    oracle: &mut O,
    index: u64,
) -> ValueAtIndexProof<N, H, A> {
    let mut it = oracle
        .create_oracle_access_iterator::<ProofForIndexIterator>(index)
        .expect("must get iterator for neighbours");
    let proof: ValueAtIndexProof<N, H, A> =
        UsizeDeserializable::from_iter(&mut it).expect("must deserialize neighbours");
    assert_eq!(proof.proof.existing.index, index);

    proof
}

#[cfg(feature = "testing")]
impl<const N: usize, H: FlatStorageHasher, const R: bool> serde::Serialize
    for FlatStorageBacking<N, H, R, Global>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        struct SerProxy<'a, const N: usize, H: FlatStorageHasher> {
            pub leaves: &'a BTreeMap<u64, FlatStorageLeaf<N>>,
            pub empty_hashes: &'a [Bytes32],
            pub hashes: &'a BTreeMap<u32, BTreeMap<u64, Bytes32>>,
            pub next_free_slot: u64,
            pub key_lookup: &'a BTreeMap<Bytes32, u64>,
            _marker: core::marker::PhantomData<H>,
        }

        let proxy = SerProxy {
            leaves: &self.leaves,
            empty_hashes: &self.empty_hashes,
            hashes: &self.hashes.0,
            next_free_slot: self.next_free_slot,
            key_lookup: &self.key_lookup,
            _marker: core::marker::PhantomData::<H>,
        };

        proxy.serialize(serializer)
    }
}

#[cfg(feature = "testing")]
impl<'de, const N: usize, H: FlatStorageHasher, const R: bool> serde::Deserialize<'de>
    for FlatStorageBacking<N, H, R, Global>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct DeProxy<const N: usize, H: FlatStorageHasher> {
            pub leaves: BTreeMap<u64, FlatStorageLeaf<N>>,
            pub empty_hashes: Vec<Bytes32>,
            pub hashes: BTreeMap<u32, BTreeMap<u64, Bytes32>>,
            pub next_free_slot: u64,
            pub key_lookup: BTreeMap<Bytes32, u64>,
            _marker: core::marker::PhantomData<H>,
        }

        let proxy: DeProxy<N, H> = DeProxy::deserialize(deserializer)?;

        Ok(Self {
            leaves: proxy.leaves,
            empty_hashes: proxy.empty_hashes,
            hashes: HashesStore(proxy.hashes),
            next_free_slot: proxy.next_free_slot,
            key_lookup: proxy.key_lookup,
            _marker: core::marker::PhantomData,
        })
    }
}

// First index is depth (0 is root, N is leaf)
// Second index is the position in that level
#[derive(Debug, Clone)]
pub struct HashesStore<A: Allocator + Clone>(pub BTreeMap<u32, BTreeMap<u64, Bytes32, A>, A>);

impl<A: Allocator + Clone> HashesStore<A> {
    fn new_in(allocator: A) -> Self {
        Self(BTreeMap::new_in(allocator))
    }
}

#[derive(Debug, Clone)]
pub struct FlatStorageBacking<
    const N: usize,
    H: FlatStorageHasher,
    const RANDOMIZED: bool,
    A: Allocator + Clone = Global,
> {
    pub leaves: BTreeMap<u64, FlatStorageLeaf<N>, A>,
    // Indexed by depth (0 is root, N is leaf)
    pub empty_hashes: Vec<Bytes32, A>,
    pub hashes: HashesStore<A>,
    pub next_free_slot: u64,
    pub key_lookup: BTreeMap<Bytes32, u64, A>,
    _marker: core::marker::PhantomData<H>,
}

#[derive(Clone)]
pub struct LeafProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub index: u64,
    pub leaf: FlatStorageLeaf<N>,
    pub path: Box<[Bytes32; N], A>,
    _marker: core::marker::PhantomData<H>,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> LeafProof<N, H, A> {
    pub fn new(index: u64, leaf: FlatStorageLeaf<N>, path: Box<[Bytes32; N], A>) -> Self {
        Self {
            index,
            leaf,
            path,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> core::fmt::Debug for LeafProof<N, H, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct(core::any::type_name::<Self>())
            .field("index", &self.index)
            .field("leaf", &self.leaf)
            .field("path", &self.path)
            .finish()
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable for LeafProof<N, H, A> {
    const USIZE_LEN: usize = <u64 as UsizeSerializable>::USIZE_LEN
        + <FlatStorageLeaf<N> as UsizeSerializable>::USIZE_LEN
        + <Bytes32 as UsizeSerializable>::USIZE_LEN * N;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        let it = ExactSizeChain::new(
            UsizeSerializable::iter(&self.index),
            ExactSizeChainN::new(
                UsizeSerializable::iter(&self.leaf),
                self.path
                    .each_ref()
                    .map(|el| Some(UsizeSerializable::iter(el))),
            ),
        );

        it
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for LeafProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let index = UsizeDeserializable::from_iter(src)?;
        let leaf = UsizeDeserializable::from_iter(src)?;
        let mut path = Box::new_in([Bytes32::ZERO; N], A::default());
        for dst in path.iter_mut() {
            *dst = UsizeDeserializable::from_iter(src)?;
        }

        Ok(Self {
            index,
            leaf,
            path,
            _marker: core::marker::PhantomData,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExistingReadProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub existing: LeafProof<N, H, A>,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for ExistingReadProof<N, H, A>
{
    const USIZE_LEN: usize = <LeafProof<N, H> as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        UsizeSerializable::iter(&self.existing)
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for ExistingReadProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let existing = UsizeDeserializable::from_iter(src)?;

        Ok(Self { existing })
    }
}

#[derive(Debug, Clone)]
pub struct NewReadProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub previous: LeafProof<N, H, A>,
    pub next: LeafProof<N, H, A>,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for NewReadProof<N, H, A>
{
    const USIZE_LEN: usize = <LeafProof<N, H, A> as UsizeSerializable>::USIZE_LEN * 2;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.previous),
            UsizeSerializable::iter(&self.next),
        )
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for NewReadProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let previous = UsizeDeserializable::from_iter(src)?;
        let next = UsizeDeserializable::from_iter(src)?;

        Ok(Self { previous, next })
    }
}

#[derive(Debug, Clone)]
pub struct NewWriteProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub previous: LeafProof<N, H, A>,
    pub next: LeafProof<N, H, A>,
    pub new_insert: LeafProof<N, H, A>,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for NewWriteProof<N, H, A>
{
    const USIZE_LEN: usize = <LeafProof<N, H, A> as UsizeSerializable>::USIZE_LEN * 3;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.previous),
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.next),
                UsizeSerializable::iter(&self.new_insert),
            ),
        )
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for NewWriteProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let previous = UsizeDeserializable::from_iter(src)?;
        let next = UsizeDeserializable::from_iter(src)?;
        let new_insert = UsizeDeserializable::from_iter(src)?;

        Ok(Self {
            previous,
            next,
            new_insert,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExistingWriteProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub existing: LeafProof<N, H, A>,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for ExistingWriteProof<N, H, A>
{
    const USIZE_LEN: usize = <LeafProof<N, H, A> as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        UsizeSerializable::iter(&self.existing)
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for ExistingWriteProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let existing = UsizeDeserializable::from_iter(src)?;

        Ok(Self { existing })
    }
}

#[repr(u32)]
#[derive(Debug, Clone)]
pub enum ReadValueWithProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    Existing {
        proof: ExistingReadProof<N, H, A>,
    } = 0,
    New {
        requested_key: Bytes32,
        proof: NewReadProof<N, H, A>,
    } = 1,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for ReadValueWithProof<N, H, A>
{
    // worst case
    const USIZE_LEN: usize = <u32 as UsizeSerializable>::USIZE_LEN
        + <Bytes32 as UsizeSerializable>::USIZE_LEN
        + <NewReadProof<N, H, A> as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        let it = match self {
            Self::Existing { proof } => Either::Left(ExactSizeChain::new(
                UsizeSerializable::iter(&0u32),
                UsizeSerializable::iter(proof),
            )),
            Self::New {
                requested_key,
                proof,
            } => Either::Right(ExactSizeChain::new(
                UsizeSerializable::iter(&1u32),
                ExactSizeChain::new(
                    UsizeSerializable::iter(requested_key),
                    UsizeSerializable::iter(proof),
                ),
            )),
        };

        it
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for ReadValueWithProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let discr: u32 = UsizeDeserializable::from_iter(src)?;
        match discr {
            0 => {
                let proof = UsizeDeserializable::from_iter(src)?;
                let new = Self::Existing { proof };
                Ok(new)
            }
            1 => {
                let requested_key = UsizeDeserializable::from_iter(src)?;
                let proof = UsizeDeserializable::from_iter(src)?;

                let new = Self::New {
                    requested_key,
                    proof,
                };
                Ok(new)
            }
            _ => Err(internal_error!("ReadValueWithProof deserialization failed")),
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone)]
pub enum WriteValueWithProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    Existing { proof: ExistingWriteProof<N, H, A> } = 0,
    New { proof: NewWriteProof<N, H, A> } = 1,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for WriteValueWithProof<N, H, A>
{
    // worst case
    const USIZE_LEN: usize = <u32 as UsizeSerializable>::USIZE_LEN
        + <NewWriteProof<N, H, A> as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        match self {
            Self::Existing { proof } => Either::Left(ExactSizeChain::new(
                UsizeSerializable::iter(&0u32),
                UsizeSerializable::iter(proof),
            )),
            Self::New { proof } => Either::Right(ExactSizeChain::new(
                UsizeSerializable::iter(&1u32),
                UsizeSerializable::iter(proof),
            )),
        }
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for WriteValueWithProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let discr: u32 = UsizeDeserializable::from_iter(src)?;
        match discr {
            0 => {
                let proof = UsizeDeserializable::from_iter(src)?;
                let new = Self::Existing { proof };
                Ok(new)
            }
            1 => {
                let proof = UsizeDeserializable::from_iter(src)?;

                let new = Self::New { proof };
                Ok(new)
            }
            _ => Err(internal_error!(
                "WriteValueWithProof deserialization failed"
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValueAtIndexProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub proof: ExistingReadProof<N, H, A>,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for ValueAtIndexProof<N, H, A>
{
    // worst case
    const USIZE_LEN: usize = <ExistingReadProof<N, H, A> as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        UsizeSerializable::iter(&self.proof)
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for ValueAtIndexProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let proof = UsizeDeserializable::from_iter(src)?;
        let new = Self { proof };

        Ok(new)
    }
}

pub fn verify_proof_for_root<const N: usize, H: FlatStorageHasher, A: Allocator>(
    hasher: &mut H,
    proof: &LeafProof<N, H, A>,
    root: &Bytes32,
) -> bool {
    let computed = recompute_root_from_proof(hasher, proof);

    &computed == root
}

pub fn recompute_root_from_proof<const N: usize, H: FlatStorageHasher, A: Allocator>(
    hasher: &mut H,
    proof: &LeafProof<N, H, A>,
) -> Bytes32 {
    let leaf_hash = hasher.hash_leaf(&proof.leaf);

    let mut current = leaf_hash;
    let mut index = proof.index;
    for path in proof.path.iter() {
        let (left, right) = if index & 1 == 0 {
            // current is left
            (&current, path)
        } else {
            (path, &current)
        };
        let next = hasher.hash_node(left, right);
        current = next;
        index >>= 1;
    }
    assert!(index == 0);

    current
}

pub fn compute_empty_hashes<const N: usize, H: FlatStorageHasher, A: Allocator>(
    hasher: &mut H,
    allocator: A,
) -> Box<[Bytes32; N], A> {
    let mut result = Box::new_in([Bytes32::ZERO; N], allocator);
    let empty_leaf = FlatStorageLeaf::<N>::empty();
    let empty_leaf_hash = hasher.hash_leaf(&empty_leaf);
    result[0] = empty_leaf_hash;
    let mut previous = empty_leaf_hash;
    for i in 0..(N - 1) {
        let node_hash = hasher.hash_node(&previous, &previous);
        result[i + 1] = node_hash;
        previous = node_hash;
    }

    result
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Clone, const RANDOMIZED: bool>
    FlatStorageBacking<N, H, RANDOMIZED, A>
{
    #[cfg(not(feature = "testing"))]
    fn new_position(&self) -> u64 {
        self.next_free_slot + 1
    }

    #[cfg(feature = "testing")]
    fn new_position(&mut self) -> u64 {
        use rand::*;
        if RANDOMIZED {
            let mut rng = rand::rng();
            let mut pos: Option<u64> = None;
            let max: u128 = 1 << N;
            let max = u64::try_from(max).unwrap_or(u64::MAX);
            while pos.is_none() {
                let i: u64 = rng.random_range(0..max);
                if !self.leaves.contains_key(&i) {
                    pos = Some(i)
                }
            }
            let pos = pos.unwrap();
            if pos > self.next_free_slot {
                self.next_free_slot = pos + 1;
                assert!(!self.leaves.contains_key(&self.next_free_slot));
            }
            pos
        } else {
            let pos = self.next_free_slot;
            self.next_free_slot += 1;
            pos
        }
    }

    #[cfg(not(feature = "testing"))]
    fn initial_positions(len: u64) -> (Vec<u64>, u64) {
        (Vec::from_iter(2..(len + 2)), len + 2)
    }

    #[cfg(feature = "testing")]
    fn initial_positions(len: u64) -> (Vec<u64>, u64) {
        use std::collections::HashSet;

        use rand::*;
        if RANDOMIZED {
            let mut rng = rand::rng();
            let mut positions: HashSet<u64> = HashSet::new();
            while (positions.len() as u64) < len {
                let i: u64 = rng.random_range(2..u64::MAX);
                positions.insert(i);
            }
            let positions: Vec<u64> = positions.drain().collect();
            let max = positions.iter().fold(1u64, |l, r| l.max(*r));
            (positions, max + 1)
        } else {
            (Vec::from_iter(2..(len + 2)), len + 2)
        }
    }

    pub fn new_in(allocator: A) -> Self {
        Self::new_in_with_leaves(allocator.clone(), Vec::new_in(allocator.clone()))
    }

    pub fn new_in_with_leaves(allocator: A, mut leaves_vec: Vec<(Bytes32, Bytes32), A>) -> Self {
        let (mut positions, next) = Self::initial_positions(leaves_vec.len() as u64);
        leaves_vec.sort_by(|(kl, _), (kr, _)| kl.cmp(kr));

        let start_guard = FlatStorageLeaf::<N> {
            key: Bytes32::from_byte_fill(0),
            value: Bytes32::ZERO,
            next: if leaves_vec.is_empty() {
                1
            } else {
                *positions.first().unwrap()
            },
        };

        let end_guard = FlatStorageLeaf::<N> {
            key: Bytes32::from_byte_fill(0xff),
            value: Bytes32::ZERO,
            next: 1,
        };

        let mut leaves: BTreeMap<u64, FlatStorageLeaf<N>, A> = BTreeMap::new_in(allocator.clone());
        leaves.insert(0, start_guard);
        leaves.insert(1, end_guard);
        // This will mark the end
        positions.push(0);
        positions.windows(2).zip(leaves_vec).for_each(|(p, leaf)| {
            let pos = p[0];
            let next = if p[1] == 0 { 1 } else { p[0] };
            leaves.insert(
                pos,
                FlatStorageLeaf::<N> {
                    key: leaf.0,
                    value: leaf.1,
                    next,
                },
            );
        });

        let empty_leaf = FlatStorageLeaf::<N>::empty();
        let mut hasher = H::new();
        let empty_leaf_hash = hasher.hash_leaf(&empty_leaf);

        let mut empty_hashes = Vec::with_capacity_in(N + 1, allocator.clone());
        empty_hashes.push(empty_leaf_hash);
        for _ in 0..N {
            let previous = empty_hashes.top().unwrap();
            let node_hash = hasher.hash_node(&previous, &previous);
            empty_hashes.push(node_hash);
        }
        empty_hashes.reverse();

        // hash all the way to the root
        let mut hashes: HashesStore<A> = HashesStore::new_in(allocator.clone());
        let mut leaf_hashes: BTreeMap<u64, Bytes32, A> = BTreeMap::new_in(allocator.clone());
        // leaves are at depth N , we first populate them
        leaves.iter().for_each(|(pos, leaf)| {
            let hash = hasher.hash_leaf(leaf);
            leaf_hashes.insert(*pos, hash);
        });
        hashes.0.insert(N as u32, leaf_hashes);

        // Next we insert the nodes
        // We only insert a node if any of its children is non-empty
        for depth in (0..=(N - 1)).rev() {
            let mut current_level: BTreeMap<u64, Bytes32, A> = BTreeMap::new_in(allocator.clone());
            let next_level = hashes.0.get(&(depth as u32 + 1)).unwrap();
            for (idx, h) in next_level.iter() {
                let (l, r) = if idx & 1 == 0 {
                    // idx is left leaf
                    let right_hash = next_level
                        .get(&(*idx + 1))
                        .unwrap_or_else(|| empty_hashes.get_mut(depth + 1).unwrap());
                    (h, right_hash)
                } else {
                    // idx is right leaf
                    let left_hash = next_level
                        .get(&(*idx - 1))
                        .unwrap_or_else(|| empty_hashes.get(depth + 1).unwrap());
                    (left_hash, h)
                };
                let parent_index = idx / 2;
                let parent_hash = hasher.hash_node(l, r);
                current_level.insert(parent_index, parent_hash);
            }

            hashes.0.insert(depth as u32, current_level);
        }

        let mut key_lookup = BTreeMap::new_in(allocator);
        key_lookup.insert(start_guard.key, 0);
        key_lookup.insert(end_guard.key, 1);
        leaves.iter().for_each(|(k, v)| {
            key_lookup.insert(v.key, *k);
        });

        Self {
            leaves,
            empty_hashes,
            hashes,
            next_free_slot: next,
            key_lookup,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn root(&self) -> &Bytes32 {
        &self.hashes.0.get(&0).unwrap().get(&0).unwrap()
    }

    pub fn verify_proof<AA: Allocator>(&self, hasher: &mut H, proof: &LeafProof<N, H, AA>) -> bool {
        verify_proof_for_root(hasher, proof, &self.root())
    }

    pub fn get_index_for_existing(&self, key: &Bytes32) -> u64 {
        let Some(existing) = self.key_lookup.get(key).copied() else {
            panic!("expected existing leaf for key {:?}", key);
        };

        existing
    }

    pub fn get_prev_index(&self, key: &Bytes32) -> u64 {
        let (_, previous) = self.key_lookup.range(..=key).next_back().unwrap();
        *previous
    }

    pub fn get_proof_for_position(&self, position: u64) -> LeafProof<N, H, A>
    where
        A: Default,
    {
        let leaf = self
            .leaves
            .get(&position)
            .copied()
            .unwrap_or(FlatStorageLeaf::empty());
        let mut path = Box::new_in([Bytes32::ZERO; N], A::default());
        let mut index = position;
        for (i, dst) in path.iter_mut().enumerate() {
            let sibling_index = index ^ 1;
            let level = self.hashes.0.get(&((N - i) as u32)).unwrap();
            *dst = *level
                .get(&sibling_index)
                .unwrap_or(&self.empty_hashes[N - i]);
            // Compute parent index
            index >>= 1;
        }
        assert!(index == 0);

        let proof = LeafProof {
            leaf,
            path,
            index: position,
            _marker: core::marker::PhantomData,
        };

        debug_assert!(self.verify_proof(&mut H::new(), &proof));

        proof
    }

    fn insert_at_position(&mut self, position: u64, leaf: FlatStorageLeaf<N>) {
        // we assume that it was pre-linked
        let mut hasher = H::new();
        let leaf_hash = hasher.hash_leaf(&leaf);

        if let Some(existing) = self.leaves.get_mut(&position) {
            if !RANDOMIZED {
                assert!(position < self.next_free_slot);
            }
            assert!(existing.key == leaf.key);
            *existing = leaf;
            let leaf_hashes = self.hashes.0.get_mut(&(N as u32)).unwrap();
            *leaf_hashes.get_mut(&position).unwrap() = leaf_hash;
        } else {
            if !RANDOMIZED {
                assert_eq!(position + 1, self.next_free_slot);
            }
            self.leaves.insert(position, leaf);
            if !RANDOMIZED {
                assert_eq!(self.leaves.len(), position as usize + 1);
            }
            let leaf_hashes = self.hashes.0.get_mut(&(N as u32)).unwrap();
            leaf_hashes.insert(position, leaf_hash);
            if !RANDOMIZED {
                assert_eq!(leaf_hashes.len(), position as usize + 1);
            }
            self.key_lookup.insert(leaf.key, position);
            if !RANDOMIZED {
                assert_eq!(self.key_lookup.len(), position as usize + 1);
            }
        };

        let mut current = leaf_hash;
        let mut index = position as usize;
        for i in (1..=N).rev() {
            let sibling_idx = index ^ 1;
            let level = self.hashes.0.get(&(i as u32)).unwrap();
            let sibling_hash = level
                .get(&(sibling_idx as u64))
                .unwrap_or(&self.empty_hashes[i]);
            let (left, right) = if index & 1 == 0 {
                (&current, sibling_hash)
            } else {
                (sibling_hash, &current)
            };
            let new_hash = hasher.hash_node(left, right);
            current = new_hash;
            // Compute parent index
            index >>= 1;
            // Update parent hash
            let parent_level = self.hashes.0.get_mut(&((i - 1) as u32)).unwrap();
            parent_level.insert(index as u64, new_hash);
        }

        assert!(index == 0);
    }

    pub fn get_value(&self, key: &Bytes32) -> Option<Bytes32> {
        self.key_lookup
            .get(key)
            .copied()
            .map(|existing| self.leaves.get(&existing).unwrap().value)
    }

    pub fn get(&self, key: &Bytes32) -> ReadValueWithProof<N, H, A>
    where
        A: Default,
    {
        if let Some(existing) = self.key_lookup.get(key).copied() {
            let existing = self.get_proof_for_position(existing);

            ReadValueWithProof::Existing {
                proof: ExistingReadProof { existing },
            }
        } else {
            let (_, previous) = self.key_lookup.range(..=key).next_back().unwrap();
            let (_, next) = self.key_lookup.range(key..).next().unwrap();
            let previous = self.get_proof_for_position(*previous);
            let next = self.get_proof_for_position(*next);

            ReadValueWithProof::New {
                requested_key: *key,
                proof: NewReadProof { previous, next },
            }
        }
    }

    pub fn insert(&mut self, key: &Bytes32, value: &Bytes32) -> WriteValueWithProof<N, H, A>
    where
        A: Default,
    {
        if let Some(existing_pos) = self.key_lookup.get(key).copied() {
            // get proof before updating
            let existing = self.get_proof_for_position(existing_pos);
            // update
            let mut existing_leaf = *self.leaves.get(&existing_pos).unwrap();
            existing_leaf.key = *key;
            existing_leaf.value = *value;
            self.insert_at_position(existing_pos, existing_leaf);

            WriteValueWithProof::Existing {
                proof: ExistingWriteProof { existing },
            }
        } else {
            let (_, &previous_pos) = self.key_lookup.range(..=key).next_back().unwrap();
            let (_, &next_pos) = self.key_lookup.range(key..).next().unwrap();
            let insert_pos = self.new_position();
            // we take "previous" leaf, then re-link it
            let previous = self.get_proof_for_position(previous_pos);
            let mut previous_leaf = previous.leaf;
            previous_leaf.next = insert_pos;
            // and insert back
            self.insert_at_position(previous_pos, previous_leaf);
            let next = self.get_proof_for_position(next_pos);
            // and now get a proof for position and insert it

            let next_pos_proof = self.get_proof_for_position(insert_pos);
            assert!(next_pos_proof.leaf == FlatStorageLeaf::empty());
            let new_leaf = FlatStorageLeaf {
                key: *key,
                value: *value,
                next: next_pos,
            };

            self.insert_at_position(insert_pos, new_leaf);

            WriteValueWithProof::New {
                proof: NewWriteProof {
                    previous,
                    next,
                    new_insert: next_pos_proof,
                },
            }
        }
    }
}

pub const TESTING_TREE_HEIGHT: usize = 64;
pub type TestingTree<const RANDOMIZED: bool> =
    FlatStorageBacking<TESTING_TREE_HEIGHT, Blake2sStorageHasher, RANDOMIZED>;

#[cfg(test)]
mod test {

    use super::*;
    use proptest::{prelude::*, sample::Index};
    use ruint::aliases::{B160, U256};
    use std::{any, collections::HashMap, ops};
    use zk_ee::common_structs::derive_flat_storage_key;
    use zk_ee::{system::NullLogger, system_io_oracle::dyn_usize_iterator::DynUsizeIterator};

    fn hex_bytes(s: &str) -> Bytes32 {
        let s = s.strip_prefix("0x").unwrap_or(s);
        assert_eq!(s.len(), 64);

        let mut bytes = [0_u8; 32];
        for (i, &byte) in s.as_bytes().iter().enumerate() {
            let hex_digit = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("Invalid byte in hex string"),
            };
            let bit_shift = if i % 2 == 0 { 4 } else { 0 };
            bytes[i / 2] += hex_digit << bit_shift;
        }
        Bytes32::from_array(bytes)
    }

    #[test]
    fn test_create() {
        let tree = TestingTree::<false>::new_in(Global);
        let mut hasher = Blake2sStorageHasher::new();

        // Test reference hash values.
        assert_eq!(
            tree.empty_hashes[TESTING_TREE_HEIGHT],
            hex_bytes("0xe3cdc93b3c2beb30f6a7c7cc45a32da012df9ae1be880e2c074885cb3f4e1e53")
        );
        assert_eq!(
            [
                *tree
                    .hashes
                    .0
                    .get(&(TESTING_TREE_HEIGHT as u32))
                    .unwrap()
                    .get(&0)
                    .unwrap(),
                *tree
                    .hashes
                    .0
                    .get(&(TESTING_TREE_HEIGHT as u32))
                    .unwrap()
                    .get(&1)
                    .unwrap(),
            ],
            [
                hex_bytes("0x9903897e51baa96a5ea51b4c194d3e0c6bcf20947cce9fd646dfb4bf754c8d28"),
                hex_bytes("0xb35299e7564e05e335094c02064bccf83d58745b417874b1fee3f523ec2007a9"),
            ]
        );
        assert_eq!(
            tree.empty_hashes[TESTING_TREE_HEIGHT - 1],
            hex_bytes("0xc45bfaf4bb5d0fee27d3178b8475155a07a1fa8ada9a15133a9016f7d0435f0f")
        );
        assert_eq!(
            tree.empty_hashes[1],
            hex_bytes("0xb720fe53e6bd4e997d967b8649e10036802a4fd3aca6d7dcc43ed9671f41cb31")
        );
        assert_eq!(
            *tree.root(),
            hex_bytes("0x90a83ead2ba2194fbbb0f7cd2a017e36cfb4891513546d943a7282c2844d4b6b")
        );

        let start_guard_proof = tree.get(&Bytes32::ZERO);
        let ReadValueWithProof::Existing { proof } = start_guard_proof else {
            panic!()
        };
        assert!(tree.verify_proof(&mut hasher, &proof.existing));
        assert!(proof.existing.leaf.key == Bytes32::ZERO);
        let end_guard_proof = tree.get(&Bytes32::MAX);
        let ReadValueWithProof::Existing { proof } = end_guard_proof else {
            panic!()
        };
        assert!(tree.verify_proof(&mut hasher, &proof.existing));
        assert!(proof.existing.leaf.key == Bytes32::MAX);

        // Check that mutating a Merkle path in the proof invalidates it.
        let mut mutated_proof = proof.existing;
        *mutated_proof.path.last_mut().unwrap() = Bytes32::zero();
        assert!(!tree.verify_proof(&mut hasher, &mutated_proof));
    }

    #[test]
    fn test_insert() {
        let mut tree = TestingTree::<false>::new_in(Global);
        let mut hasher = Blake2sStorageHasher::new();

        let initial_root = *tree.root();
        let next_available = tree.next_free_slot;
        let key_to_insert = Bytes32::from_byte_fill(0x01);
        let value_to_insert = Bytes32::from_byte_fill(0x10);
        let new_leaf_proof = tree.insert(&key_to_insert, &value_to_insert);
        let new_root = *tree.root();

        assert_eq!(
            new_root,
            hex_bytes("0x08da20879eebed16fbd14e50b427bb97c8737aa860e6519877757e238df83a15")
        );

        let WriteValueWithProof::New {
            proof:
                NewWriteProof {
                    previous,
                    next,
                    new_insert,
                },
        } = new_leaf_proof
        else {
            panic!()
        };
        assert!(new_insert.leaf == FlatStorageLeaf::empty());
        assert!(verify_proof_for_root(&mut hasher, &previous, &initial_root));
        let insert_pos = new_insert.index;
        assert!(previous.index < next_available);
        assert!(previous.leaf.key < key_to_insert);
        let mut previous = previous;
        previous.leaf.next = insert_pos;
        let mut new_intermediate = recompute_root_from_proof(&mut hasher, &previous);
        assert!(verify_proof_for_root(&mut hasher, &next, &new_intermediate));
        assert!(next.index < next_available);
        assert!(next.leaf.key > key_to_insert);
        new_intermediate = recompute_root_from_proof(&mut hasher, &next);
        assert!(verify_proof_for_root(
            &mut hasher,
            &new_insert,
            &new_intermediate
        ));
        assert!(new_insert.index == next_available);

        let mut new_insert = new_insert;
        new_insert.leaf.key = key_to_insert;
        new_insert.leaf.value = value_to_insert;
        new_insert.leaf.next = next.index;
        new_intermediate = recompute_root_from_proof(&mut hasher, &new_insert);
        assert_eq!(new_intermediate, new_root);
    }

    #[test]
    fn test_insert_many_and_update() {
        let mut tree = TestingTree::<false>::new_in(Global);
        let mut hasher = Blake2sStorageHasher::new();

        let next_available = tree.next_free_slot;
        let key_to_insert_0 = Bytes32::from_byte_fill(0x01);
        let value_to_insert_0 = Bytes32::from_byte_fill(0x10);
        let _ = tree.insert(&key_to_insert_0, &value_to_insert_0);
        let key_to_insert_1 = Bytes32::from_byte_fill(0x02);
        let value_to_insert_1 = Bytes32::from_byte_fill(0x20);
        let _ = tree.insert(&key_to_insert_1, &value_to_insert_1);

        let initial_root = *tree.root();
        let value_to_insert = Bytes32::from_byte_fill(0x33);
        let exisint_leaf_proof = tree.insert(&key_to_insert_0, &value_to_insert);
        let new_root = *tree.root();

        assert_eq!(
            initial_root,
            hex_bytes("0xf227612db17b44a5c9a2ebd0e4ff2dbe91aa05f3198d09f0bcfd6ef16c1d28c8")
        );
        assert_eq!(
            new_root,
            hex_bytes("0x81a600569c2cda27c7ae4773255acc70ac318a49404fa1035a7734a3aaa82589")
        );

        let WriteValueWithProof::Existing {
            proof: ExistingWriteProof { existing },
        } = exisint_leaf_proof
        else {
            panic!()
        };
        assert!(existing.leaf.key == key_to_insert_0);
        assert!(existing.leaf.value == value_to_insert_0);
        assert!(existing.index == next_available);
        assert!(verify_proof_for_root(&mut hasher, &existing, &initial_root));
        let mut existing = existing;
        existing.leaf.value = value_to_insert;
        assert!(verify_proof_for_root(&mut hasher, &existing, &new_root));
    }

    fn to_be_bytes(value: u64) -> Bytes32 {
        Bytes32::from_u256_be(&U256::try_from(value).unwrap())
    }

    #[test]
    fn test_key_ordering() {
        let mut tree = TestingTree::<false>::new_in(Global);
        let key_to_insert_0 = to_be_bytes(0xc0ffeefe);
        let value_to_insert_0 = Bytes32::from_byte_fill(0x10);
        let key_to_insert_1 = to_be_bytes(0xdeadbeef);
        let value_to_insert_1 = Bytes32::from_byte_fill(0x20);
        assert!(key_to_insert_0 < key_to_insert_1);

        let _ = tree.insert(&key_to_insert_0, &value_to_insert_0);
        let _ = tree.insert(&key_to_insert_1, &value_to_insert_1);
        assert_eq!(tree.leaves.get(&0).unwrap().next, 2);
        assert_eq!(tree.leaves.get(&2).unwrap().next, 3);
        assert_eq!(tree.leaves.get(&3).unwrap().next, 1);
        assert_eq!(
            *tree.root(),
            hex_bytes("0xc90465eddad7cc858a2fbf61013d7051c143887a887e5a7a19344ac32151b207")
        );
    }

    impl<const R: bool> IOOracle for TestingTree<R> {
        type MarkerTiedIterator<'a> = Box<dyn ExactSizeIterator<Item = usize>>;

        fn create_oracle_access_iterator<'a, M: OracleIteratorTypeMarker>(
            &'a mut self,
            init_value: M::Params,
        ) -> Result<Self::MarkerTiedIterator<'a>, InternalError> {
            match any::TypeId::of::<M>() {
                a if a == any::TypeId::of::<ExactIndexIterator>() => {
                    let flat_key = unsafe {
                        *(&init_value as *const M::Params)
                            .cast::<<ExactIndexIterator as OracleIteratorTypeMarker>::Params>()
                    };
                    let existing = self.get_index_for_existing(&flat_key);
                    let iterator = DynUsizeIterator::from_owned(existing);
                    Ok(Box::new(iterator))
                }
                a if a == any::TypeId::of::<ProofForIndexIterator>() => {
                    let index = unsafe {
                        *(&init_value as *const M::Params)
                            .cast::<<ProofForIndexIterator as OracleIteratorTypeMarker>::Params>()
                    };
                    let existing = self.get_proof_for_position(index);
                    let proof = ValueAtIndexProof {
                        proof: ExistingReadProof { existing },
                    };

                    let iterator = DynUsizeIterator::from_owned(proof);

                    Ok(Box::new(iterator))
                }
                a if a == any::TypeId::of::<PrevIndexIterator>() => {
                    let flat_key = unsafe {
                        *(&init_value as *const M::Params)
                            .cast::<<PrevIndexIterator as OracleIteratorTypeMarker>::Params>()
                    };
                    let prev_index = self.get_prev_index(&flat_key);
                    let iterator = DynUsizeIterator::from_owned(prev_index);
                    Ok(Box::new(iterator))
                }
                _ => panic!("unexpected request: {}", any::type_name::<M>()),
            }
        }
    }

    fn test_verifying_batch_proof(
        tree: &mut TestingTree<false>,
        entries: &[(WarmStorageKey, Option<Bytes32>)],
    ) {
        let mut tree_commitment = FlatStorageCommitment::<TESTING_TREE_HEIGHT> {
            root: *tree.root(),
            next_free_slot: tree.next_free_slot,
        };

        let entries_for_verification = entries.iter().map(|(key, new_value)| {
            let flat_key = derive_flat_storage_key(&key.address, &key.key);
            let initial_value = tree.get_value(&flat_key);
            let is_new_storage_slot = initial_value.is_none();
            let initial_value = initial_value.unwrap_or_default();
            let enriched_value = WarmStorageValue {
                initial_value,
                current_value: new_value.as_ref().copied().unwrap_or(initial_value),
                initial_value_used: true,
                is_new_storage_slot,
                // The fields below are not used during verification
                value_at_the_start_of_tx: initial_value,
                changes_stack_depth: 0,
                last_accessed_at_tx_number: None,
                pubdata_diff_bytes: 0,
            };
            (*key, enriched_value)
        });
        let entries_for_verification: Vec<_> = entries_for_verification.collect();

        tree_commitment
            .verify_and_apply_batch(
                tree,
                entries_for_verification.into_iter(),
                Global,
                &mut NullLogger,
            )
            .unwrap();

        // Apply changes to the tree.
        for (key, new_value) in entries {
            let Some(new_value) = new_value else {
                continue;
            };
            let flat_key = derive_flat_storage_key(&key.address, &key.key);
            tree.insert(&flat_key, new_value);
        }

        assert_eq!(tree_commitment.root, *tree.root());
        assert_eq!(tree_commitment.next_free_slot, tree.next_free_slot);
    }

    #[test]
    fn verifying_small_batch_proof() {
        let key_0 = WarmStorageKey {
            address: B160::ZERO,
            key: Bytes32::zero(),
        };
        let key_1 = WarmStorageKey {
            address: B160::default(),
            key: Bytes32::from_byte_fill(1),
        };
        let key_2 = WarmStorageKey {
            address: B160::default(),
            key: Bytes32::from_byte_fill(2),
        };
        let key_f = WarmStorageKey {
            address: B160::default(),
            key: Bytes32::from_byte_fill(0x0f),
        };

        let mut tree = TestingTree::new_in(Global);

        // Only missing reads
        test_verifying_batch_proof(&mut tree, &[(key_0, None), (key_1, None), (key_2, None)]);

        // Only inserts
        test_verifying_batch_proof(
            &mut tree,
            &[
                (key_0, Some(to_be_bytes(1))),
                (key_1, Some(to_be_bytes(2))),
                (key_2, Some(to_be_bytes(3))),
            ],
        );

        // Updates and reads
        test_verifying_batch_proof(
            &mut tree,
            &[
                (key_1, Some(to_be_bytes(123456))),
                (key_0, None), // existing read
                (key_2, Some(to_be_bytes(654321))),
                (key_f, None), // missing read
            ],
        );

        // Updates and inserts
        test_verifying_batch_proof(
            &mut tree,
            &[
                (key_1, Some(to_be_bytes(123456))),
                (key_f, Some(to_be_bytes(u64::MAX))),
                (key_0, Some(to_be_bytes(777))),
            ],
        );
    }

    fn uniform_bytes() -> impl Strategy<Value = Bytes32> {
        proptest::array::uniform32(proptest::num::u8::ANY).prop_map(Bytes32::from_array)
    }

    fn non_zero_bytes() -> impl Strategy<Value = Bytes32> {
        uniform_bytes().prop_filter("zero", |bytes| !bytes.is_zero())
    }

    fn uniform_full_key() -> impl Strategy<Value = WarmStorageKey> {
        let uniform_address =
            proptest::array::uniform20(proptest::num::u8::ANY).prop_map(B160::from_be_bytes);
        (uniform_address, uniform_bytes())
            .prop_map(|(address, key)| WarmStorageKey { address, key })
    }

    fn gen_entries() -> impl Strategy<Value = Vec<(WarmStorageKey, Option<Bytes32>)>> {
        let value = proptest::option::of(non_zero_bytes());
        proptest::collection::vec((uniform_full_key(), value), 0..=100)
    }

    fn gen_previous_entries(
        size: ops::Range<usize>,
    ) -> impl Strategy<Value = Vec<(WarmStorageKey, Bytes32)>> {
        proptest::collection::vec((uniform_full_key(), uniform_bytes()), size)
    }

    fn gen_reads_and_updates() -> impl Strategy<Value = Vec<(Index, Option<Bytes32>)>> {
        let value = proptest::option::of(non_zero_bytes());
        proptest::collection::vec((any::<Index>(), value), 0..=100)
    }

    proptest! {
        #[test]
        fn verifying_larger_batch_proof_for_empty_tree(entries in gen_entries()) {
            let mut tree = TestingTree::new_in(Global);
            test_verifying_batch_proof(&mut tree, &entries);
        }

        #[test]
        fn verifying_larger_batch_proof(
            prev_entries in gen_previous_entries(0..100),
            new_entries in gen_entries(),
        ) {
            let mut tree = TestingTree::new_in(Global);
            for (key, value) in &prev_entries {
                tree.insert(&derive_flat_storage_key(&key.address, &key.key), value);
            }

            test_verifying_batch_proof(&mut tree, &new_entries);
        }

        #[test]
        fn verifying_larger_batch_proof_with_updates(
            prev_entries in gen_previous_entries(1..100), // We need non-empty prev entries to select reads / updates
            new_entries in gen_entries(),
            reads_and_updates in gen_reads_and_updates(),
        ) {
            let mut tree = TestingTree::new_in(Global);
            for (key, value) in &prev_entries {
                tree.insert(&derive_flat_storage_key(&key.address, &key.key), value);
            }

            // Should be deduplicated to maintain the batch verification contract
            let reads_and_updates: HashMap<_, _> = reads_and_updates
                .into_iter()
                .map(|(idx, value)| {
                    let &(key, _) = idx.get(&prev_entries);
                    (key, value)
                })
                .collect();
            let mut all_entries = new_entries;
            all_entries.extend(reads_and_updates);
            test_verifying_batch_proof(&mut tree, &all_entries);
        }
    }
}
