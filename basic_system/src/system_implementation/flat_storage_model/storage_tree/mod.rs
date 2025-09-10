// We want to implement a flat storage (so address + storage slot under address for a single homogeneous key).
// We should care about potential attacks on the particular path, because one can make access to particular key more expensive
// by accessing specially crafted storage slot index under controlled address to make long common prefix, so we can not just
// use some dynamically-growable tree as it would lead to 160 bits common prefix attacks at complexity 2^80 that is unacceptable.

use alloc::alloc::Global;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::alloc::Allocator;
use crypto::MiniDigest;
use either::Either;
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::common_structs::state_root_view::StateRootView;
use zk_ee::common_structs::{WarmStorageKey, WarmStorageValue};
use zk_ee::internal_error;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::{
    kv_markers::{ExactSizeChain, ExactSizeChainN, UsizeDeserializable, UsizeSerializable},
    memory::stack_trait::Stack,
    system::logger::Logger,
    system_io_oracle::*,
    types_config::EthereumIOTypesConfig,
    utils::Bytes32,
};

// Note: all zeroes is well-defined for empty array slot, as we will insert two guardian values upon creation
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct FlatStorageLeafWithNextKey<const N: usize> {
    pub key: Bytes32,
    pub value: Bytes32,
    pub next_key: Bytes32,
}

impl<const N: usize> UsizeSerializable for FlatStorageLeafWithNextKey<N> {
    const USIZE_LEN: usize = <Bytes32 as UsizeSerializable>::USIZE_LEN * 3;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.key),
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.value),
                UsizeSerializable::iter(&self.next_key),
            ),
        )
    }
}

impl<const N: usize> UsizeDeserializable for FlatStorageLeafWithNextKey<N> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let key = UsizeDeserializable::from_iter(src)?;
        let value = UsizeDeserializable::from_iter(src)?;
        let next_key = UsizeDeserializable::from_iter(src)?;

        let new = Self {
            key,
            value,
            next_key,
        };

        Ok(new)
    }
}

impl<const N: usize> FlatStorageLeafWithNextKey<N> {
    pub const fn empty() -> Self {
        Self {
            key: Bytes32::ZERO,
            value: Bytes32::ZERO,
            next_key: Bytes32::ZERO,
        }
    }

    pub fn is_empty(&self) -> bool {
        self == &Self::empty()
    }
}

pub trait DigestFriendlyLeaf {
    fn persistent_digest_fn<D: MiniDigest>(&'_ self) -> Option<impl FnOnce(&mut D) + '_>;
    fn updated_digest_fn<D: MiniDigest>(&'_ self) -> impl FnOnce(&mut D) + '_;
}

pub trait FlatStorageHasher: 'static + Send + Sync + Default + core::fmt::Debug {
    fn persisted_leaf_hash(&self, leaf: &impl DigestFriendlyLeaf) -> Option<Bytes32>;
    fn updated_leaf_hash(&self, leaf: &impl DigestFriendlyLeaf) -> Bytes32;
    fn hash_node(&self, left_node: &Bytes32, right_node: &Bytes32) -> Bytes32;
}

impl<const N: usize> DigestFriendlyLeaf for FlatStorageLeafWithNextKey<N> {
    fn persistent_digest_fn<D: MiniDigest>(&'_ self) -> Option<impl FnOnce(&mut D) + '_> {
        Some(self.updated_digest_fn())
    }
    fn updated_digest_fn<D: MiniDigest>(&'_ self) -> impl FnOnce(&mut D) + '_ {
        |digest: &mut D| {
            digest.update(self.key.as_u8_array_ref());
            digest.update(self.value.as_u8_array_ref());
            digest.update(self.next_key.as_u8_array_ref());
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Blake2sStorageHasher;

impl FlatStorageHasher for Blake2sStorageHasher {
    fn persisted_leaf_hash(&self, leaf: &impl DigestFriendlyLeaf) -> Option<Bytes32> {
        leaf.persistent_digest_fn().map(|el| {
            let mut hasher = crypto::blake2s::Blake2s256::new();
            (el)(&mut hasher);
            let hasher = hasher.finalize();
            let mut dst = Bytes32::uninit();
            unsafe {
                dst.assume_init_mut()
                    .as_u8_array_mut()
                    .copy_from_slice(hasher.as_slice());
                dst.assume_init()
            }
        })
    }

    fn updated_leaf_hash(&self, leaf: &impl DigestFriendlyLeaf) -> Bytes32 {
        let el = leaf.updated_digest_fn();
        let mut hasher = crypto::blake2s::Blake2s256::new();
        (el)(&mut hasher);
        let hasher = hasher.finalize();
        let mut dst = Bytes32::uninit();
        unsafe {
            dst.assume_init_mut()
                .as_u8_array_mut()
                .copy_from_slice(hasher.as_slice());
            dst.assume_init()
        }
    }

    fn hash_node(&self, left_node: &Bytes32, right_node: &Bytes32) -> Bytes32 {
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(left_node.as_u8_array_ref());
        hasher.update(right_node.as_u8_array_ref());
        let hasher = hasher.finalize();
        let mut dst = Bytes32::uninit();
        unsafe {
            dst.assume_init_mut()
                .as_u8_array_mut()
                .copy_from_slice(hasher.as_slice());
            dst.assume_init()
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct FlatStorageCommitment<const N: usize> {
    pub root: Bytes32,
    pub next_free_slot: u64,
    pub empty_slots_stack: SlotsStackState,
}

impl<const N: usize> UsizeSerializable for FlatStorageCommitment<N> {
    const USIZE_LEN: usize = <Bytes32 as UsizeSerializable>::USIZE_LEN
        + <u64 as UsizeSerializable>::USIZE_LEN
        + <SlotsStackState as UsizeSerializable>::USIZE_LEN;
    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.root),
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.next_free_slot),
                UsizeSerializable::iter(&self.empty_slots_stack),
            ),
        )
    }
}

impl<const N: usize> UsizeDeserializable for FlatStorageCommitment<N> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let root = UsizeDeserializable::from_iter(src)?;
        let next_free_slot = UsizeDeserializable::from_iter(src)?;
        let empty_slots_stack = UsizeDeserializable::from_iter(src)?;

        let new = Self {
            root,
            next_free_slot,
            empty_slots_stack,
        };

        Ok(new)
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct SlotsStackState {
    pub state_commitment: Bytes32,
    pub num_elements: u64,
}

impl UsizeSerializable for SlotsStackState {
    const USIZE_LEN: usize =
        <Bytes32 as UsizeSerializable>::USIZE_LEN + <u64 as UsizeSerializable>::USIZE_LEN;
    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.state_commitment),
            UsizeSerializable::iter(&self.num_elements),
        )
    }
}

impl UsizeDeserializable for SlotsStackState {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let state_commitment = UsizeDeserializable::from_iter(src)?;
        let num_elements = UsizeDeserializable::from_iter(src)?;

        let new = Self {
            state_commitment,
            num_elements,
        };

        Ok(new)
    }
}

impl SlotsStackState {
    pub const fn is_empty(&self) -> bool {
        self.num_elements == 0
    }

    pub fn pop<O: IOOracle>(&mut self, oracle: &mut O) -> u64 {
        assert!(self.num_elements > 0);
        // we need to get a preimage
        let mut it = oracle
            .create_oracle_access_iterator::<EmptySlotsStackStateIterator>((
                self.state_commitment,
                self.num_elements,
            ))
            .expect("must get iterator for neighbours");
        let previous_state: Bytes32 =
            UsizeDeserializable::from_iter(&mut it).expect("must deserialize previous stack state");
        let index_to_use: u64 = UsizeDeserializable::from_iter(&mut it)
            .expect("must deserialize index popped from stack");

        // ensure that it matches
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(previous_state.as_u8_array_ref());
        hasher.update(index_to_use.to_le_bytes());
        let new_state = Bytes32::from_array(hasher.finalize());
        assert_eq!(new_state, self.state_commitment);

        self.state_commitment = previous_state;
        self.num_elements -= 1;

        index_to_use
    }

    pub fn push(&mut self, slot: u64) {
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(self.state_commitment.as_u8_array_ref());
        hasher.update(slot.to_le_bytes());

        self.num_elements += 1;
        self.state_commitment = Bytes32::from_array(hasher.finalize());
    }
}

#[derive(Clone)]
pub struct LeafCacheRecord<const N: usize, A: Allocator + Clone> {
    pub persisted_leaf: Option<LeafProof<N, Blake2sStorageHasher, A>>,
    pub modification: LeafUpdateRecord,
}

impl<const N: usize, A: Allocator + Clone> core::fmt::Debug for LeafCacheRecord<N, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("")
            .field(&self.persisted_leaf)
            .field(&self.modification)
            .finish()
    }
}

impl<const N: usize, A: Allocator + Clone> LeafCacheRecord<N, A> {
    pub const fn new_from_persisted(persisted_leaf: LeafProof<N, Blake2sStorageHasher, A>) -> Self {
        Self {
            persisted_leaf: Some(persisted_leaf),
            modification: LeafUpdateRecord {
                key: None,
                value: None,
                next_key: None,
            },
        }
    }

    pub fn current_key(&self) -> &Bytes32 {
        if let Some(updated_key) = self.modification.key.as_ref() {
            updated_key
        } else {
            &self
                .persisted_leaf
                .as_ref()
                .expect("must be persistent")
                .leaf
                .key
        }
    }

    pub fn current_next_key(&self) -> &Bytes32 {
        if let Some(updated_next_key) = self.modification.next_key.as_ref() {
            updated_next_key
        } else {
            &self
                .persisted_leaf
                .as_ref()
                .expect("must be persistent")
                .leaf
                .next_key
        }
    }

    pub fn current_value(&self) -> &Bytes32 {
        if let Some(updated_value) = self.modification.value.as_ref() {
            updated_value
        } else {
            &self
                .persisted_leaf
                .as_ref()
                .expect("must be persistent")
                .leaf
                .value
        }
    }

    pub fn persistent_key(&self) -> Option<&Bytes32> {
        self.persisted_leaf.as_ref().map(|el| &el.leaf.key)
    }

    pub fn persistent_value(&self) -> Option<&Bytes32> {
        self.persisted_leaf.as_ref().map(|el| &el.leaf.value)
    }

    pub fn persistent_next_key(&self) -> Option<&Bytes32> {
        self.persisted_leaf.as_ref().map(|el| &el.leaf.next_key)
    }

    pub fn update_key(&mut self, new: Bytes32) {
        self.modification.key = Some(new);
    }

    pub fn update_next_key(&mut self, new: Bytes32) {
        self.modification.next_key = Some(new);
    }

    pub fn update_value(&mut self, new: Bytes32) {
        self.modification.value = Some(new);
    }

    pub fn mark_deleted(&mut self) {
        self.modification.key = Some(Bytes32::ZERO);
        self.modification.value = Some(Bytes32::ZERO);
        self.modification.next_key = Some(Bytes32::ZERO);
    }

    pub fn is_modified(&self) -> bool {
        if let Some(persistent) = self.persisted_leaf.as_ref() {
            if let Some(key) = self.modification.key.as_ref() {
                if key != &persistent.leaf.key {
                    return true;
                }
            }
            if let Some(value) = self.modification.value.as_ref() {
                if value != &persistent.leaf.value {
                    return true;
                }
            }
            if let Some(next_key) = self.modification.next_key.as_ref() {
                if next_key != &persistent.leaf.next_key {
                    return true;
                }
            }

            false
        } else {
            assert!(self.modification.key.is_some());
            assert!(self.modification.value.is_some());
            assert!(self.modification.next_key.is_some());

            true
        }
    }
}

impl<const N: usize, A: Allocator + Clone> DigestFriendlyLeaf for LeafCacheRecord<N, A> {
    fn persistent_digest_fn<D: MiniDigest>(&'_ self) -> Option<impl FnOnce(&mut D) + '_> {
        self.persisted_leaf.as_ref().map(|el| {
            |digest: &mut D| {
                digest.update(el.leaf.key.as_u8_array_ref());
                digest.update(el.leaf.value.as_u8_array_ref());
                digest.update(el.leaf.next_key.as_u8_array_ref());
            }
        })
    }
    fn updated_digest_fn<D: MiniDigest>(&'_ self) -> impl FnOnce(&mut D) + '_ {
        |digest: &mut D| {
            digest.update(self.current_key().as_u8_array_ref());
            digest.update(self.current_value().as_u8_array_ref());
            digest.update(self.current_next_key().as_u8_array_ref());
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LeafUpdateRecord {
    pub key: Option<Bytes32>,
    pub value: Option<Bytes32>,
    pub next_key: Option<Bytes32>,
}

impl LeafUpdateRecord {
    pub const fn insert(key: Bytes32, value: Bytes32) -> Self {
        Self {
            key: Some(key),
            value: Some(value),
            next_key: None,
        }
    }
}

// Get another leaf such that it's key is just the next smaller one that requested flat key
fn get_or_insert_previous_for_flat_key<
    'a,
    const N: usize,
    A: Allocator + Clone + Default,
    O: IOOracle,
>(
    key_to_index_cache: &'_ mut BTreeMap<Bytes32, u64, A>,
    index_to_leaf_cache: &'a mut BTreeMap<u64, LeafCacheRecord<N, A>, A>,
    flat_key: &Bytes32,
    oracle: &mut O,
    saved_next_free_slot: u64,
) -> (&'a mut LeafCacheRecord<N, A>, u64) {
    let previous_idx = get_prev_index::<O>(oracle, flat_key);
    // it must be some existing index
    assert!(previous_idx < saved_next_free_slot);

    let entry = index_to_leaf_cache.entry(previous_idx).or_insert_with(|| {
        let leaf = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(oracle, previous_idx)
            .proof
            .existing;
        let existing = key_to_index_cache.insert(leaf.leaf.key, previous_idx);
        assert!(existing.is_none());

        LeafCacheRecord::new_from_persisted(leaf)
    });
    assert!(entry.current_key() < flat_key);

    (entry, previous_idx)
}

fn get_for_existing_flat_key<'a, const N: usize, A: Allocator + Clone + Default, O: IOOracle>(
    key_to_index_cache: &'_ mut BTreeMap<Bytes32, u64, A>,
    index_to_leaf_cache: &'a mut BTreeMap<u64, LeafCacheRecord<N, A>, A>,
    flat_key: &Bytes32,
    oracle: &mut O,
    saved_next_free_slot: u64,
) -> (&'a mut LeafCacheRecord<N, A>, u64) {
    let index = if let Some(index) = key_to_index_cache.get(flat_key).copied() {
        index
    } else {
        get_index::<O>(oracle, flat_key)
    };
    assert!(index < saved_next_free_slot);

    let entry = index_to_leaf_cache.entry(index).or_insert_with(|| {
        let leaf = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(oracle, index)
            .proof
            .existing;
        // check the key upon inserting into cache
        assert_eq!(&leaf.leaf.key, flat_key);
        let existing = key_to_index_cache.insert(leaf.leaf.key, index);
        assert!(existing.is_none());

        LeafCacheRecord::new_from_persisted(leaf)
    });

    (entry, index)
}

fn remove_for_existing_flat_key<'a, const N: usize, A: Allocator + Clone + Default, O: IOOracle>(
    key_to_index_cache: &'_ mut BTreeMap<Bytes32, u64, A>,
    index_to_leaf_cache: &'a mut BTreeMap<u64, LeafCacheRecord<N, A>, A>,
    flat_key: &Bytes32,
    oracle: &mut O,
    saved_next_free_slot: u64,
) -> (&'a mut LeafCacheRecord<N, A>, u64) {
    let index = if let Some(index) = key_to_index_cache.remove(flat_key) {
        index
    } else {
        get_index::<O>(oracle, flat_key)
    };
    assert!(index < saved_next_free_slot);

    let entry = index_to_leaf_cache.entry(index).or_insert_with(|| {
        let leaf = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(oracle, index)
            .proof
            .existing;
        // check the key upon inserting into cache
        assert_eq!(&leaf.leaf.key, flat_key);

        LeafCacheRecord::new_from_persisted(leaf)
    });

    (entry, index)
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
        let mut index_to_leaf_cache =
            BTreeMap::<u64, LeafCacheRecord<N, A>, A>::new_in(allocator.clone());
        let mut empty_slots_cache: BTreeSet<u64, A> = BTreeSet::new_in(allocator.clone());

        // If there was no IO, just return
        if source.clone().next().is_none() {
            return Ok(());
        }

        let reads_iter = source
            .clone()
            .filter(|(_, v)| v.current_value == v.initial_value);
        // NOTE: here we degrade something like fresh write of 0 into 0 into read, and we will
        // automatically get it as "read initial" as we have `is_new_storage_slot == true`

        // save it for later
        let saved_next_free_slot = self.next_free_slot;

        for (key, value) in reads_iter {
            let flat_key = derive_flat_storage_key(&key.address, &key.key);
            // reads
            let expect_new = value.is_new_storage_slot;
            assert!(value.initial_value_used);
            if expect_new {
                assert_eq!(
                    value.initial_value,
                    Bytes32::ZERO,
                    "initial value of empty slot must be trivial"
                );

                // TODO: debug implementation for B160 uses global alloc, which panics in ZKsync OS
                #[cfg(not(target_arch = "riscv32"))]
                let _ = logger.write_fmt(format_args!(
                    "checking empty read for address = {:?}, key = {:?}\n",
                    &key.address, &key.key,
                ));

                let (entry, _index) = get_or_insert_previous_for_flat_key(
                    &mut key_to_index_cache,
                    &mut index_to_leaf_cache,
                    &flat_key,
                    oracle,
                    saved_next_free_slot,
                );
                // we expect current read to be empty, so we also check `next_key`
                assert!(entry.current_next_key() > &flat_key);

                // we will perform all merkle path verifications separately at the end
            } else {
                // TODO: debug implementation for B160 uses global alloc, which panics in ZKsync OS
                #[cfg(not(target_arch = "riscv32"))]
                let _ = logger.write_fmt(format_args!(
                    "checking existing read for address = {:?}, key = {:?}, value = {:?}\n",
                    &key.address, &key.key, value.current_value,
                ));

                let (entry, _index) = get_for_existing_flat_key(
                    &mut key_to_index_cache,
                    &mut index_to_leaf_cache,
                    &flat_key,
                    oracle,
                    saved_next_free_slot,
                );
                assert_eq!(entry.current_value(), &value.current_value);
            }
        }

        // NOTE: we effectively build a linked list backed by the fixed capacity (merkle tree) array,
        // and our `key_to_index_cache` is just another representation of it

        let mut num_total_writes = 0;
        let mut num_appends = 0;

        // we will want to separately update, delete and insert
        let updates_iter = source.clone().filter(|(_, v)| {
            v.current_value != v.initial_value
                && v.is_new_storage_slot == false
                && v.current_value.is_zero() == false
        });

        let deletes_iter = source.clone().filter(|(_, v)| {
            v.current_value != v.initial_value
                && v.is_new_storage_slot == false
                && v.current_value.is_zero() == true
        });

        let inserts_iter = source
            .clone()
            .filter(|(_, v)| v.current_value != v.initial_value && v.is_new_storage_slot == true);

        // updates are simple - we expect leaves with such key to be present in the tree
        for (key, value) in updates_iter {
            num_total_writes += 1;
            let flat_key = derive_flat_storage_key(&key.address, &key.key);

            // TODO: debug implementation for B160 uses global alloc, which panics in ZKsync OS
            #[cfg(not(target_arch = "riscv32"))]
            let _ = logger.write_fmt(format_args!(
                "applying repeated write for address = {:?}, key = {:?}, value {:?} -> {:?}\n",
                &key.address, &key.key, &value.initial_value, &value.current_value
            ));
            // here we branch because we COULD have requested this one as a read witness before

            let (entry, _index) = get_for_existing_flat_key(
                &mut key_to_index_cache,
                &mut index_to_leaf_cache,
                &flat_key,
                oracle,
                saved_next_free_slot,
            );
            assert_eq!(entry.current_value(), &value.initial_value);
            debug_assert!(value.current_value.is_zero() == false);
            entry.update_value(value.current_value);
        }

        // NOTE: Any re-linking can only happen below. We also add another restriction to our implementation -
        // we want all "previous index" answers to come from the state of the tree that is before any updates,
        // so it's convenient for node implementation - we can cache key -> index part before applying an update,
        // and avoid recomputing updates after every insert. Instead we will carefully relink below

        // NOTE: flat keys come in random order, so we should be extra careful with re-linking

        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        enum RelinkData {
            Deleted {
                previous_index: u64,
                deletion_index: u64,
            },
            Insert {
                previous_index: u64,
                insertion_index: u64,
            },
        }

        let mut relinked_slots = BTreeMap::<Bytes32, RelinkData, A>::new_in(allocator.clone());

        for (key, value) in deletes_iter {
            num_total_writes += 1;
            let flat_key = derive_flat_storage_key(&key.address, &key.key);

            // TODO: debug implementation for B160 uses global alloc, which panics in ZKsync OS
            #[cfg(not(target_arch = "riscv32"))]
            let _ = logger.write_fmt(format_args!(
                "applying delete for address = {:?}, key = {:?}\n",
                &key.address, &key.key
            ));
            // here we branch because we COULD have requested this one as a read witness before

            let (entry, index) = remove_for_existing_flat_key(
                &mut key_to_index_cache,
                &mut index_to_leaf_cache,
                &flat_key,
                oracle,
                saved_next_free_slot,
            );
            assert_eq!(entry.current_value(), &value.initial_value);
            debug_assert!(value.current_value.is_zero());
            let next_key_to_use = *entry.current_next_key();
            // we saved "next key", and can now delete it completely
            entry.mark_deleted();

            let (entry, previous_index) = get_or_insert_previous_for_flat_key(
                &mut key_to_index_cache,
                &mut index_to_leaf_cache,
                &flat_key,
                oracle,
                saved_next_free_slot,
            );
            assert_ne!(index, previous_index);
            assert_eq!(entry.current_next_key(), &flat_key);

            // We store a key that has to be re-linked, and two indexes - a slot that previously pointed to it
            relinked_slots.insert(
                next_key_to_use,
                RelinkData::Deleted {
                    previous_index,
                    deletion_index: index,
                },
            );

            let is_new = empty_slots_cache.insert(index);
            assert!(is_new);
        }

        // and last one - insert. We will reuse slots
        for (key, value) in inserts_iter {
            num_total_writes += 1;
            num_appends += 1;
            let flat_key = derive_flat_storage_key(&key.address, &key.key);

            // TODO: debug implementation for B160 uses global alloc, which panics in ZKsync OS
            #[cfg(not(target_arch = "riscv32"))]
            let _ = logger.write_fmt(format_args!(
                "applying insert for address = {:?}, key = {:?}, value {:?} -> {:?}\n",
                &key.address, &key.key, &value.initial_value, &value.current_value
            ));

            // since it's new, we always ask for some previous key
            let (entry, previous_index) = get_or_insert_previous_for_flat_key(
                &mut key_to_index_cache,
                &mut index_to_leaf_cache,
                &flat_key,
                oracle,
                saved_next_free_slot,
            );
            assert!(entry.current_next_key() > &flat_key);

            // and we will either append it to the tree, or will insert it
            // instead of some empty one in the middle

            let no_empty_slots = empty_slots_cache.is_empty() && self.empty_slots_stack.is_empty();
            let inserted_pos = if no_empty_slots {
                // we have to append
                let index = self.next_free_slot;
                self.next_free_slot += 1;

                // insert only in one of the caches
                let existing = key_to_index_cache.insert(flat_key, index);
                assert!(existing.is_none());
                let cache_record = LeafCacheRecord {
                    persisted_leaf: None,
                    modification: LeafUpdateRecord::insert(flat_key, value.current_value),
                };
                let existing = index_to_leaf_cache.insert(index, cache_record);
                assert!(existing.is_none());

                index
            } else {
                // we require the prover to provide any empty leaf somewhere in the existing subtree
                if empty_slots_cache.is_empty() == false {
                    // we already have proofs and elements that we can reuse
                    let index_to_use = empty_slots_cache.pop_last().expect("exists");
                    let cached_record = index_to_leaf_cache
                        .get_mut(&index_to_use)
                        .expect("must be in cache");
                    assert!(cached_record.current_key().is_zero());
                    assert!(cached_record.current_value().is_zero());

                    cached_record.update_key(flat_key);
                    cached_record.update_value(value.current_value);

                    index_to_use
                } else {
                    // we pop a slot from the stack of previously empty ones
                    let empty_slot_index = self.empty_slots_stack.pop(oracle);
                    assert!(empty_slot_index < saved_next_free_slot);
                    let leaf = get_proof_for_index(oracle, empty_slot_index);

                    // check that it's deleted
                    assert!(leaf.proof.existing.leaf.key.is_zero());
                    assert!(leaf.proof.existing.leaf.value.is_zero());
                    assert!(leaf.proof.existing.leaf.next_key.is_zero());

                    let existing = key_to_index_cache.insert(flat_key, empty_slot_index);
                    assert!(existing.is_none());

                    let mut cache_record = LeafCacheRecord::new_from_persisted(leaf.proof.existing);
                    // update
                    cache_record.update_key(flat_key);
                    cache_record.update_value(value.current_value);

                    // insert into cache along with update record
                    let existing = index_to_leaf_cache.insert(empty_slot_index, cache_record);
                    assert!(existing.is_none());

                    empty_slot_index
                }
            };

            relinked_slots.insert(
                flat_key,
                RelinkData::Insert {
                    previous_index,
                    insertion_index: inserted_pos,
                },
            );
        }

        // now we can re-link in a cascading manner. Iteration over BTreeMap will give us keys in ascending order,
        // so we just chain

        let mut maybe_chain = None;
        for (key_to_relink, relink_data) in relinked_slots.into_iter() {
            match relink_data {
                RelinkData::Deleted {
                    previous_index,
                    deletion_index,
                } => {
                    // we deleted a leaf that had `next_key = key_to_relink`, and `previous_index` was pointing to it,
                    // and `deletion_index` was it's location

                    // chaining (cascading action) - maybe `previous_index` was one that we relinked previously
                    let source_index = match maybe_chain.take() {
                        Some((beginning, end)) if end == previous_index => {
                            maybe_chain = Some((beginning, deletion_index));
                            beginning
                        }
                        _ => {
                            // we should start a new chain
                            maybe_chain = Some((previous_index, deletion_index));
                            previous_index
                        }
                    };

                    // We deleted, so we should use `next_key` of deleted one as new `next_key` of it's `previous` one
                    let entry = index_to_leaf_cache
                        .get_mut(&source_index)
                        .expect("must be present in cache");
                    assert!(entry.current_next_key() < &key_to_relink);
                    entry.update_next_key(key_to_relink);
                }
                RelinkData::Insert {
                    previous_index,
                    insertion_index,
                } => {
                    // we inserted `key_to_relink`, and `previous_index` was one where `key < key_to_relink` and `next_key > key_to_relink`,
                    // and `insertion_index` in a location where it was inserted

                    // chaining (cascading action) - maybe `previous_index` was one that we relinked previously
                    let source_index = match maybe_chain.take() {
                        Some((source, end)) if end == previous_index => {
                            maybe_chain = Some((insertion_index, end));
                            source
                        }
                        _ => {
                            // we should start a new chain
                            maybe_chain = Some((insertion_index, previous_index));
                            previous_index
                        }
                    };
                    let entry = index_to_leaf_cache
                        .get_mut(&source_index)
                        .expect("must be present in cache");
                    assert!(entry.current_next_key() > &key_to_relink);
                    let t = *entry.current_next_key();
                    entry.update_next_key(key_to_relink);

                    let entry = index_to_leaf_cache
                        .get_mut(&insertion_index)
                        .expect("must be present in cache");
                    assert_eq!(entry.current_key(), &key_to_relink);
                    entry.update_next_key(t);
                }
            }
        }

        // now we potentially augment `index_to_leaf_cache` using the path for the "rightmost" tree element,
        // and recompute a binary tree by folding either computed nodes, or computed + witness

        if num_appends > 0 {
            // get rightmost path as it'll be needed anyway to append more leaves
            if index_to_leaf_cache.contains_key(&(saved_next_free_slot - 1)) == false {
                let proof = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(
                    oracle,
                    saved_next_free_slot - 1,
                );
                let cache_entry = LeafCacheRecord::new_from_persisted(proof.proof.existing);
                index_to_leaf_cache.insert(saved_next_free_slot - 1, cache_entry);
            }
        }

        // and now we join
        let hasher = Blake2sStorageHasher;
        let empty_hashes = compute_empty_hashes::<N, Blake2sStorageHasher, A>(allocator.clone());

        // now we should have fun and join the paths
        let buffer_size = index_to_leaf_cache.len() + num_appends;
        let mut current_hashes_buffer = Vec::with_capacity_in(buffer_size, allocator.clone());
        let mut next_hashes_buffer = Vec::with_capacity_in(buffer_size, allocator.clone());

        for (index, cache_record) in index_to_leaf_cache.iter() {
            let leaf_hash = hasher.persisted_leaf_hash(cache_record);
            if leaf_hash.is_none() {
                // it's an append
                assert!(*index >= saved_next_free_slot);
            }
            let updated_hash = if cache_record.is_modified() {
                Some(hasher.updated_leaf_hash(cache_record))
            } else {
                None
            };

            current_hashes_buffer.push((*index, *index, leaf_hash, updated_hash));
        }

        // then merge
        fn can_merge(pair: (u64, u64)) -> bool {
            let (a, b) = pair;
            debug_assert_ne!(a, b);
            a & !1 == b & !1
        }

        let process_single =
            |a: &(u64, u64, Option<Bytes32>, Option<Bytes32>),
             level: u32,
             dst: &mut Vec<(u64, u64, Option<Bytes32>, Option<Bytes32>), A>| {
                let (
                    index_at_current_depth,
                    absolute_leaf_index,
                    read_verification_hash,
                    update_computation_hash,
                ) = a;
                let is_left = *index_at_current_depth & 1 == 0;
                let proof = match index_to_leaf_cache.get(absolute_leaf_index) {
                    Some(cache_record) => {
                        if let Some(persisted) = cache_record.persisted_leaf.as_ref() {
                            &persisted.path[level as usize]
                        } else {
                            &empty_hashes[level as usize]
                        }
                    }
                    None => {
                        // use default
                        &empty_hashes[level as usize]
                    }
                };

                let read_path = if let Some(read_path) = read_verification_hash.as_ref() {
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

                let write_path = if let Some(write_path) = update_computation_hash.as_ref() {
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

                // recompute index for next level
                let index = *index_at_current_depth >> 1;
                dst.push((index, *absolute_leaf_index, read_path, write_path));
            };

        for level in 0..N {
            assert!(!current_hashes_buffer.is_empty());
            if current_hashes_buffer.len() == 1 {
                // just progress
                let a = &current_hashes_buffer[0];
                process_single(a, level as u32, &mut next_hashes_buffer);

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
                    process_single(a, level as u32, &mut next_hashes_buffer);
                }
            }

            if last_merged == false {
                // we need to progress last
                let a = current_hashes_buffer.last().unwrap();
                process_single(a, level as u32, &mut next_hashes_buffer);
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
            assert_eq!(num_appends, 0);
            // root should not change in such case
        }

        // we append key back into stack
        for el in empty_slots_cache.into_iter().rev() {
            self.empty_slots_stack.push(el);
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
            pub leaves: &'a BTreeMap<u64, FlatStorageLeafWithNextKey<N>>,
            pub empty_hashes: &'a [Bytes32],
            pub hashes: &'a BTreeMap<u32, BTreeMap<u64, Bytes32>>,
            pub next_free_slot: u64,
            pub key_lookup: &'a BTreeMap<Bytes32, u64>,
            pub empty_elements_stack: &'a Vec<u64>,
            _marker: core::marker::PhantomData<H>,
        }

        let proxy = SerProxy {
            leaves: &self.leaves,
            empty_hashes: &self.empty_hashes,
            hashes: &self.hashes.0,
            next_free_slot: self.next_free_slot,
            key_lookup: &self.key_lookup,
            empty_elements_stack: &self.empty_elements_stack,
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
            pub leaves: BTreeMap<u64, FlatStorageLeafWithNextKey<N>>,
            pub empty_hashes: Vec<Bytes32>,
            pub hashes: BTreeMap<u32, BTreeMap<u64, Bytes32>>,
            pub next_free_slot: u64,
            pub key_lookup: BTreeMap<Bytes32, u64>,
            pub empty_elements_stack: Vec<u64>,
            _marker: core::marker::PhantomData<H>,
        }

        let proxy: DeProxy<N, H> = DeProxy::deserialize(deserializer)?;

        Ok(Self {
            leaves: proxy.leaves,
            empty_hashes: proxy.empty_hashes,
            hashes: HashesStore(proxy.hashes),
            next_free_slot: proxy.next_free_slot,
            key_lookup: proxy.key_lookup,
            empty_elements_stack: proxy.empty_elements_stack,
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
    pub leaves: BTreeMap<u64, FlatStorageLeafWithNextKey<N>, A>,
    // Indexed by depth (0 is root, N is leaf)
    pub empty_hashes: Vec<Bytes32, A>,
    pub hashes: HashesStore<A>,
    pub next_free_slot: u64,
    pub key_lookup: BTreeMap<Bytes32, u64, A>,
    pub empty_elements_stack: Vec<u64, A>,
    _marker: core::marker::PhantomData<H>,
}

#[derive(Clone)]
pub struct LeafProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub index: u64,
    pub leaf: FlatStorageLeafWithNextKey<N>,
    pub path: Box<[Bytes32; N], A>,
    _marker: core::marker::PhantomData<H>,
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
        + <FlatStorageLeafWithNextKey<N> as UsizeSerializable>::USIZE_LEN
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
pub struct EmptyReadProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub previous: LeafProof<N, H, A>,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for EmptyReadProof<N, H, A>
{
    const USIZE_LEN: usize = <LeafProof<N, H, A> as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        UsizeSerializable::iter(&self.previous)
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for EmptyReadProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let previous = UsizeDeserializable::from_iter(src)?;

        Ok(Self { previous })
    }
}

#[derive(Debug, Clone)]
pub struct InsertProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub previous: LeafProof<N, H, A>,
    pub new_insert: LeafProof<N, H, A>,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for InsertProof<N, H, A>
{
    const USIZE_LEN: usize = <LeafProof<N, H, A> as UsizeSerializable>::USIZE_LEN * 2;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.previous),
            UsizeSerializable::iter(&self.new_insert),
        )
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for InsertProof<N, H, A>
{
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let previous = UsizeDeserializable::from_iter(src)?;
        let new_insert = UsizeDeserializable::from_iter(src)?;

        Ok(Self {
            previous,
            new_insert,
        })
    }
}

#[derive(Debug, Clone)]
pub struct UpdateProof<const N: usize, H: FlatStorageHasher, A: Allocator = Global> {
    pub existing: LeafProof<N, H, A>,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for UpdateProof<N, H, A>
{
    const USIZE_LEN: usize = <LeafProof<N, H, A> as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        UsizeSerializable::iter(&self.existing)
    }
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator + Default> UsizeDeserializable
    for UpdateProof<N, H, A>
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
    Empty {
        requested_key: Bytes32,
        proof: EmptyReadProof<N, H, A>,
    } = 1,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for ReadValueWithProof<N, H, A>
{
    // worst case
    const USIZE_LEN: usize = <u32 as UsizeSerializable>::USIZE_LEN
        + <Bytes32 as UsizeSerializable>::USIZE_LEN
        + <EmptyReadProof<N, H, A> as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        let it = match self {
            Self::Existing { proof } => Either::Left(ExactSizeChain::new(
                UsizeSerializable::iter(&0u32),
                UsizeSerializable::iter(proof),
            )),
            Self::Empty {
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

                let new = Self::Empty {
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
    Update { proof: UpdateProof<N, H, A> } = 0,
    Insert { proof: InsertProof<N, H, A> } = 1,
}

impl<const N: usize, H: FlatStorageHasher, A: Allocator> UsizeSerializable
    for WriteValueWithProof<N, H, A>
{
    // worst case
    const USIZE_LEN: usize = <u32 as UsizeSerializable>::USIZE_LEN
        + <InsertProof<N, H, A> as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        match self {
            Self::Update { proof } => Either::Left(ExactSizeChain::new(
                UsizeSerializable::iter(&0u32),
                UsizeSerializable::iter(proof),
            )),
            Self::Insert { proof } => Either::Right(ExactSizeChain::new(
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
                let new = Self::Update { proof };
                Ok(new)
            }
            1 => {
                let proof = UsizeDeserializable::from_iter(src)?;

                let new = Self::Insert { proof };
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
    proof: &LeafProof<N, H, A>,
    root: &Bytes32,
) -> bool {
    let computed = recompute_root_from_proof(proof);

    &computed == root
}

pub fn recompute_root_from_proof<const N: usize, H: FlatStorageHasher, A: Allocator>(
    proof: &LeafProof<N, H, A>,
) -> Bytes32 {
    let hasher = H::default();
    let leaf_hash = hasher.updated_leaf_hash(&proof.leaf);

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
    allocator: A,
) -> Box<[Bytes32; N], A> {
    let mut result = Box::new_in([Bytes32::ZERO; N], allocator);
    let empty_leaf = FlatStorageLeafWithNextKey::<N>::empty();
    let hasher = H::default();
    let empty_leaf_hash = hasher.updated_leaf_hash(&empty_leaf);
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
    // #[cfg(not(feature = "testing"))]
    fn new_position(&mut self) -> u64 {
        if let Some(el) = self.empty_elements_stack.pop() {
            el
        } else {
            let t = self.next_free_slot;
            self.next_free_slot += 1;

            t
        }
    }

    // #[cfg(feature = "testing")]
    // fn new_position(&mut self) -> u64 {
    //     use rand::*;
    //     if RANDOMIZED {
    //         let mut rng = rand::rng();
    //         let mut pos: Option<u64> = None;
    //         let max: u128 = 1 << N;
    //         let max = u64::try_from(max).unwrap_or(u64::MAX);
    //         while pos.is_none() {
    //             let i: u64 = rng.random_range(0..max);
    //             if !self.leaves.contains_key(&i) {
    //                 pos = Some(i)
    //             }
    //         }
    //         let pos = pos.unwrap();
    //         if pos > self.next_free_slot {
    //             self.next_free_slot = pos + 1;
    //             assert!(!self.leaves.contains_key(&self.next_free_slot));
    //         }
    //         pos
    //     } else {
    //         let pos = self.next_free_slot;
    //         self.next_free_slot += 1;
    //         pos
    //     }
    // }

    // #[cfg(not(feature = "testing"))]
    fn initial_positions(len: u64) -> (Vec<u64>, u64) {
        (Vec::from_iter(2..(len + 2)), len + 2)
    }

    // #[cfg(feature = "testing")]
    // fn initial_positions(len: u64) -> (Vec<u64>, u64) {
    //     use std::collections::HashSet;

    //     use rand::*;
    //     if RANDOMIZED {
    //         let mut rng = rand::rng();
    //         let mut positions: HashSet<u64> = HashSet::new();
    //         while (positions.len() as u64) < len {
    //             let i: u64 = rng.random_range(2..u64::MAX);
    //             positions.insert(i);
    //         }
    //         let positions: Vec<u64> = positions.drain().collect();
    //         let max = positions.iter().fold(1u64, |l, r| l.max(*r));
    //         (positions, max + 1)
    //     } else {
    //         (Vec::from_iter(2..(len + 2)), len + 2)
    //     }
    // }

    pub fn new_in(allocator: A) -> Self {
        Self::new_in_with_leaves(allocator.clone(), Vec::new_in(allocator.clone()))
    }

    pub fn new_in_with_leaves(allocator: A, mut leaves_vec: Vec<(Bytes32, Bytes32), A>) -> Self {
        let (mut positions, next) = Self::initial_positions(leaves_vec.len() as u64);
        leaves_vec.sort_by(|(kl, _), (kr, _)| kl.cmp(kr));

        let start_guard = FlatStorageLeafWithNextKey::<N> {
            key: Bytes32::ZERO,
            value: Bytes32::ZERO,
            next_key: if leaves_vec.is_empty() {
                Bytes32::MAX
            } else {
                leaves_vec[*positions.first().unwrap() as usize].0
            },
        };

        let end_guard = FlatStorageLeafWithNextKey::<N> {
            key: Bytes32::MAX,
            value: Bytes32::ZERO,
            next_key: Bytes32::MAX, // convention, never used in practice
        };

        let mut leaves: BTreeMap<u64, FlatStorageLeafWithNextKey<N>, A> =
            BTreeMap::new_in(allocator.clone());
        leaves.insert(0, start_guard);
        leaves.insert(1, end_guard);
        // This will mark the end
        positions.push(0);
        positions.windows(2).zip(&leaves_vec).for_each(|(p, leaf)| {
            let pos = p[0];
            let next = if p[1] == 0 { 1 } else { p[0] };
            let next_key = leaves_vec[next as usize].0;
            leaves.insert(
                pos,
                FlatStorageLeafWithNextKey {
                    key: leaf.0,
                    value: leaf.1,
                    next_key,
                },
            );
        });

        let empty_leaf = FlatStorageLeafWithNextKey::<N>::empty();
        let hasher = H::default();
        let empty_leaf_hash = hasher.updated_leaf_hash(&empty_leaf);

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
            let hash = hasher.updated_leaf_hash(leaf);
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

        let mut key_lookup = BTreeMap::new_in(allocator.clone());
        key_lookup.insert(start_guard.key, 0);
        key_lookup.insert(end_guard.key, 1);
        leaves.iter().for_each(|(k, v)| {
            key_lookup.insert(v.key, *k);
        });

        Self {
            leaves,
            empty_hashes,
            hashes,
            empty_elements_stack: Vec::new_in(allocator), // we pack densely
            next_free_slot: next,
            key_lookup,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn root(&self) -> &Bytes32 {
        &self.hashes.0.get(&0).unwrap().get(&0).unwrap()
    }

    pub fn stack_state_encoding(&self) -> Bytes32 {
        // no caching, but it's only used for tests
        let mut state = Bytes32::ZERO;
        for slot in self.empty_elements_stack.iter() {
            let mut hasher = crypto::blake2s::Blake2s256::new();
            hasher.update(state.as_u8_array_ref());
            hasher.update(slot.to_le_bytes());
            state = Bytes32::from_array(hasher.finalize());
        }

        state
    }

    pub fn verify_proof<AA: Allocator>(&self, proof: &LeafProof<N, H, AA>) -> bool {
        verify_proof_for_root(proof, &self.root())
    }

    pub fn get_index_for_existing(&self, key: &Bytes32) -> u64 {
        let Some(existing) = self.key_lookup.get(key).copied() else {
            panic!("expected existing leaf for key {key:?}");
        };

        existing
    }

    pub fn get_empty_slots_preimage(
        &self,
        current_state: &Bytes32,
        current_num_elements: u64,
    ) -> (Bytes32, u64) {
        assert!(
            current_state.is_zero() == false,
            "trying to request preimage for state {current_state:?}"
        );
        assert!(current_num_elements > 0);
        assert!(current_num_elements <= self.empty_elements_stack.len() as u64);
        let mut state = Bytes32::ZERO;
        for i in 0..(current_num_elements - 1) {
            let slot = self.empty_elements_stack[i as usize];
            let mut hasher = crypto::blake2s::Blake2s256::new();
            hasher.update(state.as_u8_array_ref());
            hasher.update(slot.to_le_bytes());
            state = Bytes32::from_array(hasher.finalize());
        }
        let slot = self.empty_elements_stack[(current_num_elements - 1) as usize];

        (state, slot)
    }

    pub fn get_prev_index(&self, key: &Bytes32) -> u64 {
        let (_, previous) = self.key_lookup.range(..key).next_back().unwrap();
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
            .unwrap_or(FlatStorageLeafWithNextKey::empty());
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

        debug_assert!(self.verify_proof(&proof));

        proof
    }

    pub fn get_proof_for_existing_key(&self, key: &Bytes32) -> ExistingReadProof<N, H, A>
    where
        A: Default,
    {
        let pos = self.get_index_for_existing(key);
        let proof = self.get_proof_for_position(pos);

        ExistingReadProof { existing: proof }
    }

    #[track_caller]
    fn insert_at_position(&mut self, position: u64, leaf: FlatStorageLeafWithNextKey<N>) {
        // we assume that it was pre-linked
        let hasher = H::default();
        let leaf_hash = hasher.updated_leaf_hash(&leaf);

        if let Some(existing) = self.leaves.get_mut(&position) {
            *existing = leaf;
            let leaf_hashes = self.hashes.0.get_mut(&(N as u32)).unwrap();
            *leaf_hashes.get_mut(&position).unwrap() = leaf_hash;
            if leaf.is_empty() == false {
                self.key_lookup.insert(leaf.key, position);
            }
        } else {
            self.leaves.insert(position, leaf);
            if leaf.is_empty() == false {
                assert!(self.key_lookup.contains_key(&leaf.key) == false);
                self.key_lookup.insert(leaf.key, position);
            }
            let leaf_hashes = self.hashes.0.get_mut(&(N as u32)).unwrap();
            leaf_hashes.insert(position, leaf_hash);
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
            let (_, previous) = self.key_lookup.range(..key).next_back().unwrap();
            let previous = self.get_proof_for_position(*previous);

            ReadValueWithProof::Empty {
                requested_key: *key,
                proof: EmptyReadProof { previous },
            }
        }
    }

    fn update(&mut self, key: &Bytes32, new_value: &Bytes32) {
        let Some(existing_pos) = self.key_lookup.get(key).copied() else {
            panic!("expected to update existing")
        };
        // update
        let mut existing_leaf = *self.leaves.get(&existing_pos).unwrap();
        assert_eq!(existing_leaf.key, *key);
        existing_leaf.value = *new_value;
        self.insert_at_position(existing_pos, existing_leaf);
    }

    fn delete(&mut self, key: &Bytes32, batch_bound_empty_slots: &mut BTreeSet<u64, A>) {
        let Some(existing_pos) = self.key_lookup.remove(key) else {
            panic!("Trying to delete non-existent key {key:?}");
        };

        // delete via update
        let existing_leaf = self.leaves.remove(&existing_pos).unwrap();
        let to_relink = existing_leaf.next_key;
        // insert an empty one instead
        self.insert_at_position(existing_pos, FlatStorageLeafWithNextKey::empty());
        let is_unique = batch_bound_empty_slots.insert(existing_pos);
        assert!(is_unique);

        // re-link
        let (_, &previous_pos) = self.key_lookup.range(..key).next_back().unwrap();
        let mut previous_leaf = *self.leaves.get(&previous_pos).unwrap();
        assert_eq!(previous_leaf.next_key, *key);
        previous_leaf.next_key = to_relink;
        self.insert_at_position(previous_pos, previous_leaf);
    }

    pub fn insert_dense_or_update(&mut self, key: &Bytes32, value: &Bytes32)
    where
        A: Default,
    {
        if self.key_lookup.contains_key(key) {
            self.update(key, value);
        } else {
            assert!(self.empty_elements_stack.is_empty());
            self.insert(key, value, &mut BTreeSet::new_in(A::default()));
        }
    }

    pub fn insert(
        &mut self,
        key: &Bytes32,
        value: &Bytes32,
        batch_bound_empty_slots: &mut BTreeSet<u64, A>,
    ) {
        assert!(self.key_lookup.contains_key(key) == false);
        let insert_pos = if let Some(pos) = batch_bound_empty_slots.pop_last() {
            pos
        } else if let Some(pos) = self.empty_elements_stack.pop() {
            pos
        } else {
            let t = self.next_free_slot;
            self.next_free_slot += 1;

            t
        };

        let (_, &previous_pos) = self.key_lookup.range(..key).next_back().unwrap();
        let mut previous_leaf = *self.leaves.get(&previous_pos).unwrap();
        let t = previous_leaf.next_key;
        previous_leaf.next_key = *key;
        // and insert back
        self.insert_at_position(previous_pos, previous_leaf);

        if let Some(deleted_or_empty) = self.leaves.get(&insert_pos) {
            assert_eq!(*deleted_or_empty, FlatStorageLeafWithNextKey::empty());
        }

        let new_leaf = FlatStorageLeafWithNextKey {
            key: *key,
            value: *value,
            next_key: t,
        };
        self.insert_at_position(insert_pos, new_leaf);
    }

    pub fn batch_update(&mut self, batch: impl Iterator<Item = (Bytes32, Bytes32)>)
    where
        A: Default,
    {
        // NOTE: we must not change an order of iteration (as we do not sort such order in OS code),
        // but we will self-check that iterator only contains unique values
        let mut set = BTreeSet::new_in(A::default());
        let mut ops = Vec::new_in(A::default());
        // both sort and check uniqueness
        for (k, v) in batch {
            let is_unique = set.insert(k);
            if is_unique == false {
                panic!("Batch contains duplicate entries for key {k:?}");
            }
            ops.push((k, v));
        }

        let updates: Vec<_> = ops
            .iter()
            .filter(|kv| {
                let (k, v) = kv;
                if self.key_lookup.contains_key(k) == false {
                    return false;
                }
                v.is_zero() == false
            })
            .collect();
        // println!("{} updates", updates.len());
        for (k, v) in updates.into_iter() {
            self.update(k, v);
            assert!(self.key_lookup.contains_key(k) == true);
        }

        // Collect, than apply
        let deletes: Vec<_> = ops
            .iter()
            .filter(|kv| {
                let (k, v) = kv;
                if self.key_lookup.contains_key(k) == false {
                    return false;
                }
                v.is_zero() == true
            })
            .collect();
        // println!("{} deletes", deletes.len());

        let inserts: Vec<_> = ops
            .iter()
            .filter(|kv| {
                let (k, v) = kv;
                if self.key_lookup.contains_key(k) {
                    return false;
                }
                assert!(
                    v.is_zero() == false,
                    "trying to insert a zero-value for key {k:?}"
                );

                true
            })
            .collect();
        // println!("{} inserts", inserts.len());

        let mut slots = BTreeSet::new_in(A::default());

        for (k, _) in deletes.into_iter() {
            self.delete(k, &mut slots);
            assert!(self.key_lookup.contains_key(k) == false);
        }

        for (k, v) in inserts.into_iter() {
            self.insert(k, v, &mut slots);
            assert!(self.key_lookup.contains_key(k) == true);
        }

        for el in slots.into_iter().rev() {
            self.empty_elements_stack.push(el);
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

        // // Test reference hash values.
        // assert_eq!(
        //     tree.empty_hashes[TESTING_TREE_HEIGHT],
        //     hex_bytes("0xe3cdc93b3c2beb30f6a7c7cc45a32da012df9ae1be880e2c074885cb3f4e1e53")
        // );
        // assert_eq!(
        //     [
        //         *tree
        //             .hashes
        //             .0
        //             .get(&(TESTING_TREE_HEIGHT as u32))
        //             .unwrap()
        //             .get(&0)
        //             .unwrap(),
        //         *tree
        //             .hashes
        //             .0
        //             .get(&(TESTING_TREE_HEIGHT as u32))
        //             .unwrap()
        //             .get(&1)
        //             .unwrap(),
        //     ],
        //     [
        //         hex_bytes("0x9903897e51baa96a5ea51b4c194d3e0c6bcf20947cce9fd646dfb4bf754c8d28"),
        //         hex_bytes("0xb35299e7564e05e335094c02064bccf83d58745b417874b1fee3f523ec2007a9"),
        //     ]
        // );
        // assert_eq!(
        //     tree.empty_hashes[TESTING_TREE_HEIGHT - 1],
        //     hex_bytes("0xc45bfaf4bb5d0fee27d3178b8475155a07a1fa8ada9a15133a9016f7d0435f0f")
        // );
        // assert_eq!(
        //     tree.empty_hashes[1],
        //     hex_bytes("0xb720fe53e6bd4e997d967b8649e10036802a4fd3aca6d7dcc43ed9671f41cb31")
        // );
        // assert_eq!(
        //     *tree.root(),
        //     hex_bytes("0x90a83ead2ba2194fbbb0f7cd2a017e36cfb4891513546d943a7282c2844d4b6b")
        // );

        let start_guard_proof = tree.get(&Bytes32::ZERO);
        let ReadValueWithProof::Existing { proof } = start_guard_proof else {
            panic!()
        };
        assert!(tree.verify_proof(&proof.existing));
        assert!(proof.existing.leaf.key == Bytes32::ZERO);
        let end_guard_proof = tree.get(&Bytes32::MAX);
        let ReadValueWithProof::Existing { proof } = end_guard_proof else {
            panic!()
        };
        assert!(tree.verify_proof(&proof.existing));
        assert!(proof.existing.leaf.key == Bytes32::MAX);

        // Check that mutating a Merkle path in the proof invalidates it.
        let mut mutated_proof = proof.existing;
        *mutated_proof.path.last_mut().unwrap() = Bytes32::zero();
        assert!(!tree.verify_proof(&mutated_proof));
    }

    // #[test]
    // fn test_insert() {
    //     let mut tree = TestingTree::<false>::new_in(Global);
    //     let initial_root = *tree.root();
    //     let next_available = tree.next_free_slot;
    //     let key_to_insert = Bytes32::from_byte_fill(0x01);
    //     let value_to_insert = Bytes32::from_byte_fill(0x10);
    //     let new_leaf_proof = tree.update_or_insert(&key_to_insert, &value_to_insert);
    //     let new_root = *tree.root();

    //     assert_eq!(
    //         new_root,
    //         hex_bytes("0x08da20879eebed16fbd14e50b427bb97c8737aa860e6519877757e238df83a15")
    //     );

    //     let WriteValueWithProof::Insert {
    //         proof:
    //             InsertProof {
    //                 previous,
    //                 new_insert,
    //             },
    //     } = new_leaf_proof
    //     else {
    //         panic!()
    //     };
    //     assert!(new_insert.leaf == FlatStorageLeafWithNextKey::empty());
    //     assert!(verify_proof_for_root(&previous, &initial_root));
    //     let insert_pos = new_insert.index;
    //     assert!(previous.index < next_available);
    //     assert!(previous.leaf.key < key_to_insert);
    //     let mut previous = previous;
    //     previous.leaf.next = insert_pos;
    //     let mut new_intermediate = recompute_root_from_proof(&previous);
    //     assert!(verify_proof_for_root(&next, &new_intermediate));
    //     assert!(next.index < next_available);
    //     assert!(next.leaf.key > key_to_insert);
    //     new_intermediate = recompute_root_from_proof(&next);
    //     assert!(verify_proof_for_root(&new_insert, &new_intermediate));
    //     assert!(new_insert.index == next_available);

    //     let mut new_insert = new_insert;
    //     new_insert.leaf.key = key_to_insert;
    //     new_insert.leaf.value = value_to_insert;
    //     new_insert.leaf.next = next.index;
    //     new_intermediate = recompute_root_from_proof(&new_insert);
    //     assert_eq!(new_intermediate, new_root);
    // }

    #[test]
    fn test_insert_many_and_update() {
        let mut tree = TestingTree::<false>::new_in(Global);
        let next_available = tree.next_free_slot;
        let key_to_insert_0 = Bytes32::from_byte_fill(0x01);
        let value_to_insert_0 = Bytes32::from_byte_fill(0x10);
        let key_to_insert_1 = Bytes32::from_byte_fill(0x02);
        let value_to_insert_1 = Bytes32::from_byte_fill(0x20);

        tree.batch_update(
            vec![
                (key_to_insert_0, value_to_insert_0),
                (key_to_insert_1, value_to_insert_1),
            ]
            .into_iter(),
        );

        let initial_root = *tree.root();
        let update_value = Bytes32::from_byte_fill(0x33);
        let existing_leaf_proof = tree.get_proof_for_existing_key(&key_to_insert_0);
        tree.batch_update(vec![(key_to_insert_0, update_value)].into_iter());
        let new_root = *tree.root();

        // assert_eq!(
        //     initial_root,
        //     hex_bytes("0xf227612db17b44a5c9a2ebd0e4ff2dbe91aa05f3198d09f0bcfd6ef16c1d28c8")
        // );
        // assert_eq!(
        //     new_root,
        //     hex_bytes("0x81a600569c2cda27c7ae4773255acc70ac318a49404fa1035a7734a3aaa82589")
        // );

        let existing = existing_leaf_proof.existing;
        assert!(existing.leaf.key == key_to_insert_0);
        assert!(existing.leaf.value == value_to_insert_0);
        assert!(existing.index == next_available);
        assert!(verify_proof_for_root(&existing, &initial_root));
        let mut existing = existing;
        existing.leaf.value = update_value;
        assert!(verify_proof_for_root(&existing, &new_root));
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

        tree.batch_update(
            vec![
                (key_to_insert_0, value_to_insert_0),
                (key_to_insert_1, value_to_insert_1),
            ]
            .into_iter(),
        );

        assert_eq!(
            tree.leaves.get(&0).unwrap().next_key,
            to_be_bytes(0xc0ffeefe)
        );
        assert_eq!(
            tree.leaves.get(&2).unwrap().next_key,
            to_be_bytes(0xdeadbeef)
        );
        assert_eq!(tree.leaves.get(&3).unwrap().next_key, Bytes32::MAX);
        // assert_eq!(
        //     *tree.root(),
        //     hex_bytes("0xc90465eddad7cc858a2fbf61013d7051c143887a887e5a7a19344ac32151b207")
        // );
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
                a if a == any::TypeId::of::<EmptySlotsStackStateIterator>() => {
                    let (state, num_elements) = unsafe {
                        *(&init_value as *const M::Params)
                            .cast::<<EmptySlotsStackStateIterator as OracleIteratorTypeMarker>::Params>()
                    };
                    let data = self.get_empty_slots_preimage(&state, num_elements);
                    let iterator = DynUsizeIterator::from_owned(data);
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
            empty_slots_stack: SlotsStackState {
                state_commitment: tree.stack_state_encoding(),
                num_elements: tree.empty_elements_stack.len() as u64,
            },
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

        let updates: Vec<_> = entries
            .iter()
            .filter(|el| el.1.is_some())
            .map(|(k, v)| {
                let Some(new_value) = v else {
                    panic!();
                };
                let flat_key = derive_flat_storage_key(&k.address, &k.key);

                (flat_key, *new_value)
            })
            .collect();

        tree.batch_update(updates.into_iter());

        assert_eq!(tree_commitment.next_free_slot, tree.next_free_slot);
        assert_eq!(
            tree_commitment.empty_slots_stack.num_elements,
            tree.empty_elements_stack.len() as u64
        );
        assert_eq!(
            tree_commitment.empty_slots_stack.state_commitment,
            tree.stack_state_encoding()
        );
        assert_eq!(tree_commitment.root, *tree.root());
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
        let key_0f = WarmStorageKey {
            address: B160::default(),
            key: Bytes32::from_byte_fill(0x0f),
        };
        let key_f0 = WarmStorageKey {
            address: B160::default(),
            key: Bytes32::from_byte_fill(0xf0),
        };
        let key_ff = WarmStorageKey {
            address: B160::default(),
            key: Bytes32::from_byte_fill(0xff),
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
                (key_0f, None), // missing read
            ],
        );

        // Updates and inserts
        test_verifying_batch_proof(
            &mut tree,
            &[
                (key_1, Some(to_be_bytes(123456))),
                (key_0f, Some(to_be_bytes(u64::MAX))),
                (key_0, Some(to_be_bytes(777))),
            ],
        );

        // Updates, deletes and inserts
        test_verifying_batch_proof(
            &mut tree,
            &[
                (key_1, Some(to_be_bytes(123456))),
                (key_0, Some(to_be_bytes(0))),
                (key_0f, Some(to_be_bytes(42))),
                (key_f0, Some(to_be_bytes(98765))),
            ],
        );

        // Insert one
        test_verifying_batch_proof(&mut tree, &[(key_ff, Some(to_be_bytes(123456)))]);

        // Delete it
        test_verifying_batch_proof(&mut tree, &[(key_ff, Some(to_be_bytes(0)))]);

        // Re-insert same position it
        test_verifying_batch_proof(&mut tree, &[(key_ff, Some(to_be_bytes(456)))]);
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

        // #[test]
        // fn verifying_larger_batch_proof(
        //     prev_entries in gen_previous_entries(0..100),
        //     new_entries in gen_entries(),
        // ) {
        //     let mut tree = TestingTree::new_in(Global);
        //     for (key, value) in &prev_entries {
        //         tree.update_or_insert(&derive_flat_storage_key(&key.address, &key.key), value);
        //     }

        //     test_verifying_batch_proof(&mut tree, &new_entries);
        // }

        // #[test]
        // fn verifying_larger_batch_proof_with_updates(
        //     prev_entries in gen_previous_entries(1..100), // We need non-empty prev entries to select reads / updates
        //     new_entries in gen_entries(),
        //     reads_and_updates in gen_reads_and_updates(),
        // ) {
        //     let mut tree = TestingTree::new_in(Global);
        //     for (key, value) in &prev_entries {
        //         tree.update_or_insert(&derive_flat_storage_key(&key.address, &key.key), value);
        //     }

        //     // Should be deduplicated to maintain the batch verification contract
        //     let reads_and_updates: HashMap<_, _> = reads_and_updates
        //         .into_iter()
        //         .map(|(idx, value)| {
        //             let &(key, _) = idx.get(&prev_entries);
        //             (key, value)
        //         })
        //         .collect();
        //     let mut all_entries = new_entries;
        //     all_entries.extend(reads_and_updates);
        //     test_verifying_batch_proof(&mut tree, &all_entries);
        // }
    }
}
