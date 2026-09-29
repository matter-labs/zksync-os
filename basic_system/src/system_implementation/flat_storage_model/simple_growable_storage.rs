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

use super::FLAT_STORAGE_SUBSPACE_MASK;
use alloc::alloc::Global;
use alloc::boxed::Box;
use alloc::collections::{btree_map, BTreeMap};
use alloc::vec::Vec;
#[cfg(any(target_arch = "riscv32", test))]
use blake2s_u32::{
    state_with_extended_control::Blake2RoundFunctionEvaluator, AlignedArray64,
    BLAKE2S_BLOCK_SIZE_U32_WORDS,
};
use core::alloc::Allocator;
use core::mem::MaybeUninit;
use crypto::MiniDigest;
use either::Either;
#[cfg(any(target_arch = "riscv32", test))]
use u256::byte_order::bswap32;
use zk_ee::common_structs::state_root_view::StateRootView;
use zk_ee::common_structs::FlatStorageKeyDeriver;
use zk_ee::common_structs::{WarmStorageKey, WarmStorageValue};
use zk_ee::oracle::memory_io::host::WriteQueryOutput;
use zk_ee::oracle::memory_io::{ContinuousDeserializable, ContinuousSerializable, OracleQuery};
use zk_ee::oracle::query_ids::STATE_AND_MERKLE_PATHS_SUBSPACE_MASK;
use zk_ee::utils::exact_size_chain::{ExactSizeChain, ExactSizeChainN};
use zk_ee::{internal_error, logger_log};
use zk_ee::{
    memory::stack_trait::Stack,
    oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable},
    oracle::IOOracle,
    system::{errors::internal::InternalError, logger::Logger},
    types_config::EthereumIOTypesConfig,
    utils::Bytes32,
};

mod arena;
mod batch;
mod fold;
mod key_index;

use arena::*;
use batch::*;
use fold::*;
use key_index::*;

// we use u64 types below, but in practice smaller depth will be used

pub const MIN_KEY_LEAF_MARKER_IDX: u64 = 0;
pub const MAX_KEY_LEAF_MARKER_IDX: u64 = 1;

// Note: all zeroes is well-defined for empty array slot, as we will insert two guardian values upon creation
// `#[repr(C)]`: the oracle passes it memcpy-like (`zk_ee::oracle::memory_io`)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct FlatStorageLeaf<const N: usize> {
    pub key: Bytes32,
    pub value: Bytes32,
    pub next: u64,
}

// The key, the value and the index of the next leaf, without padding
const _: () = assert!(
    core::mem::size_of::<FlatStorageLeaf<0>>() == 72
        && core::mem::align_of::<FlatStorageLeaf<0>>() == 8
);

// SAFETY: `#[repr(C)]` of fixed-width fields without padding (see above)
unsafe impl<const N: usize> ContinuousSerializable for FlatStorageLeaf<N> {}

// SAFETY: as above; any bytes form a valid leaf
unsafe impl<const N: usize> ContinuousDeserializable for FlatStorageLeaf<N> {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> Result<&'a mut Self, InternalError> {
        // SAFETY: guaranteed by the caller, and any initialized bytes form a valid value
        Ok(unsafe { &mut *this })
    }
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

#[cfg(any(target_arch = "riscv32", test))]
#[derive(Clone, Debug)]
pub struct DirectBlake2sStorageHasher {
    evaluator: Blake2RoundFunctionEvaluator,
    leaf_tail: AlignedArray64<u32, BLAKE2S_BLOCK_SIZE_U32_WORDS>,
}

#[cfg(any(target_arch = "riscv32", test))]
impl DirectBlake2sStorageHasher {
    #[inline]
    fn compress_first_block(&mut self, left: &Bytes32, right: &Bytes32, last: bool) {
        // SAFETY: Bytes32 consists of 32 initialized bytes; every u32 bit pattern is valid.
        let left = unsafe { core::mem::transmute::<Bytes32, [u32; 8]>(*left) }.map(u32::from_le);
        let right = unsafe { core::mem::transmute::<Bytes32, [u32; 8]>(*right) }.map(u32::from_le);
        *self.evaluator.get_witness_buffer() = [
            left[0], left[1], left[2], left[3], left[4], left[5], left[6], left[7], right[0],
            right[1], right[2], right[3], right[4], right[5], right[6], right[7],
        ];
        self.evaluator.reset();
        // SAFETY: the complete 64-byte input block was written above.
        unsafe {
            self.evaluator
                .run_round_function_with_byte_len::<false>(64, last);
        }
    }

    #[inline]
    fn digest(&self) -> Bytes32 {
        let words = self.evaluator.read_state_for_output().map(u32::to_le);
        // SAFETY: both types are 32 bytes and every byte pattern is a valid Bytes32.
        unsafe { core::mem::transmute::<[u32; 8], Bytes32>(words) }
    }
}

#[cfg(any(target_arch = "riscv32", test))]
impl FlatStorageHasher for DirectBlake2sStorageHasher {
    fn new() -> Self {
        // `Blake2RoundFunctionEvaluator::new()` leaves fields uninitialized on riscv32.
        // SAFETY: its fields are integer arrays and a zero-sized alignment marker.
        let mut evaluator =
            unsafe { MaybeUninit::<Blake2RoundFunctionEvaluator>::zeroed().assume_init() };
        evaluator.reset();
        Self {
            evaluator,
            leaf_tail: AlignedArray64::from_value(0),
        }
    }

    fn hash_leaf<const N: usize>(&mut self, leaf: &FlatStorageLeaf<N>) -> Bytes32 {
        #[cfg(test)]
        test::count(test::Counter::DirectLeaf);
        self.compress_first_block(&leaf.key, &leaf.value, false);
        self.leaf_tail[0] = leaf.next as u32;
        self.leaf_tail[1] = (leaf.next >> 32) as u32;
        // SAFETY: the tail is 64-byte aligned; words 2..16 remain zero.
        unsafe {
            self.evaluator
                .run_round_function_with_input_and_byte_len::<false>(&self.leaf_tail, 8, true);
        }
        self.digest()
    }

    fn hash_node(&mut self, left_node: &Bytes32, right_node: &Bytes32) -> Bytes32 {
        #[cfg(test)]
        test::count(test::Counter::DirectNode);
        self.compress_first_block(left_node, right_node, true);
        self.digest()
    }
}

#[cfg(any(target_arch = "riscv32", test))]
type CommitmentHasher = DirectBlake2sStorageHasher;
#[cfg(not(any(target_arch = "riscv32", test)))]
type CommitmentHasher = Blake2sStorageHasher;

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

/// Query for finding the previous index in the sorted flat storage for a given key
pub struct PreviousIndexQuery;

impl OracleQuery for PreviousIndexQuery {
    const QUERY_ID: u32 = STATE_AND_MERKLE_PATHS_SUBSPACE_MASK | FLAT_STORAGE_SUBSPACE_MASK;
    type Input = Bytes32;
    type Output = u64;
}

/// Query for finding the exact index of a key in the flat storage
pub struct ExactIndexQuery;

impl OracleQuery for ExactIndexQuery {
    const QUERY_ID: u32 = STATE_AND_MERKLE_PATHS_SUBSPACE_MASK | FLAT_STORAGE_SUBSPACE_MASK | 0x01;
    type Input = Bytes32;
    type Output = u64;
}

/// Query ID for requesting Merkle proof at a specific index in flat storage
pub const PROOF_FOR_INDEX_QUERY_ID: u32 =
    STATE_AND_MERKLE_PATHS_SUBSPACE_MASK | FLAT_STORAGE_SUBSPACE_MASK | 0x02;

/// Query for obtaining a Merkle proof for a value at a specific index in flat storage: the index, the
/// leaf and the path
pub struct ProofForIndexQuery<const N: usize>;

impl<const N: usize> OracleQuery for ProofForIndexQuery<N> {
    const QUERY_ID: u32 = PROOF_FOR_INDEX_QUERY_ID;
    type Input = u64;
    /// `(index, leaf, path)`
    type Output = (u64, FlatStorageLeaf<N>, [Bytes32; N]);
}

/// Oracle side of [`ProofForIndexQuery`]: the response for `proof`.
pub fn write_proof_for_index_response<const N: usize, H: FlatStorageHasher, A: Allocator>(
    proof: &LeafProof<N, H, A>,
    response: &mut Vec<u32>,
) {
    // a composite output is the concatenation of its elements
    proof.index.write_output(response);
    proof.leaf.write_output(response);
    proof.path.write_output(response);
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
    let path_ref: &[Bytes32] = &*proof.path;
    for path in path_ref.iter() {
        let path: &Bytes32 = path;
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
        leaves_vec.sort_by_key(|(k, _)| *k);

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
            let next = if p[1] == 0 { 1 } else { p[1] };
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
            panic!("expected existing leaf for key {key:?}");
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
#[path = "simple_growable_storage/tests/mod.rs"]
mod test;
