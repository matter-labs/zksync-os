use alloc::{alloc::Global, collections::BTreeMap};
use core::{alloc::Allocator, marker::PhantomData};
use storage_models::common_structs::{snapshottable_io::SnapshottableIo, PreimageCacheModel};
use zk_ee::{
    common_structs::{history_map::CacheSnapshotId, NewPreimagesPublicationStorage, PreimageType},
    execution_environment_type::ExecutionEnvironmentType,
    internal_error,
    oracle::query_ids::PREIMAGE_SUBSPACE_MASK,
    out_of_native_resources,
    system::{
        errors::{internal::InternalError, system::SystemError},
        IOResultKeeper, Resources,
    },
    types_config::EthereumIOTypesConfig,
    utils::{num_usize_words_for_u8_capacity, Bytes32, UsizeAlignedByteBox},
};

use super::cost_constants::PREIMAGE_CACHE_GET_NATIVE_COST;
use super::*;
use crate::cost_constants::blake2s_native_cost;

/// Query ID for requesting preimage data from the flat storage system
pub const FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID: u32 =
    PREIMAGE_SUBSPACE_MASK | FLAT_STORAGE_SUBSPACE_MASK;

/// `UsizeAlignedByteBox` rounds allocations to pairs of native words. Sixteen
/// bytes covers that rounding on both 32- and 64-bit targets without making
/// resource charging architecture-dependent.
const PREIMAGE_CACHE_ALLOCATION_ALIGNMENT: usize = 16;
/// Conservative allowance for the key, value, B-tree node, and allocator
/// metadata retained by each raw cache entry.
const PREIMAGE_CACHE_ENTRY_MEMORY_OVERHEAD: usize = 256;
/// The raw preimage cache may retain at most 256 MiB, including conservative
/// per-entry map and allocator overhead.
pub const MAX_PREIMAGE_CACHE_RETAINED_BYTES: usize = 256 * 1024 * 1024;
/// A single transaction may add at most half of the block cache limit.
pub const MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX: usize = MAX_PREIMAGE_CACHE_RETAINED_BYTES / 2;

const _: () =
    assert!(MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX * 2 == MAX_PREIMAGE_CACHE_RETAINED_BYTES);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct PreimageRequest {
    pub hash: Bytes32,
    pub expected_preimage_len_in_bytes: u32,
    pub preimage_type: PreimageType,
}

/// Block-scoped cache whose raw entries survive transaction and frame rollback.
///
/// Cache hits do not consume either memory budget. Before allocating a miss,
/// the cache checks both the bytes added by the current transaction and the
/// total bytes retained by the block. Counters are updated only after a
/// successful insertion.
///
/// Exceeding the transaction budget returns ordinary OON. Exceeding the block
/// cap sets `block_limit_hit_for_current_tx`; the bootloader then rolls the
/// transaction back as `BlockNativeLimitReached` without committing effects or
/// fees. `begin_new_tx` resets transaction-local accounting and flags, but the
/// raw entries and block-retained byte counter deliberately remain.
pub struct BytecodeAndAccountDataPreimagesStorage<R: Resources, A: Allocator + Clone = Global> {
    pub(crate) storage: BTreeMap<Bytes32, UsizeAlignedByteBox<A>, A>,
    pub(crate) publication_storage: NewPreimagesPublicationStorage<A>,
    pub(crate) allocator: A,
    estimated_retained_bytes: usize,
    estimated_bytes_added_in_current_tx: usize,
    tx_limit_hit_for_current_tx: bool,
    block_limit_hit_for_current_tx: bool,
    _marker: PhantomData<R>,
}

impl<R: Resources, A: Allocator + Clone> BytecodeAndAccountDataPreimagesStorage<R, A> {
    pub fn new_from_parts(allocator: A) -> Self {
        let publication_storage = NewPreimagesPublicationStorage::new_from_parts(allocator.clone());
        Self {
            storage: BTreeMap::new_in(allocator.clone()),
            publication_storage,
            allocator,
            estimated_retained_bytes: 0,
            estimated_bytes_added_in_current_tx: 0,
            tx_limit_hit_for_current_tx: false,
            block_limit_hit_for_current_tx: false,
            _marker: PhantomData,
        }
    }

    /// Used only to log ordinary OON caused by the transaction memory budget.
    pub fn tx_limit_hit_for_current_tx(&self) -> bool {
        self.tx_limit_hit_for_current_tx
    }

    /// Checked after execution so a block cap hit invalidates the transaction
    /// before any of its effects or fees are committed.
    pub fn block_limit_hit_for_current_tx(&self) -> bool {
        self.block_limit_hit_for_current_tx
    }

    /// Estimates the bytes a new entry would retain, including allocation
    /// rounding and conservative map/allocator overhead.
    pub(super) fn estimated_entry_bytes(preimage_len: usize) -> Option<usize> {
        let aligned_allocation = preimage_len
            .checked_add(PREIMAGE_CACHE_ALLOCATION_ALIGNMENT - 1)?
            / PREIMAGE_CACHE_ALLOCATION_ALIGNMENT
            * PREIMAGE_CACHE_ALLOCATION_ALIGNMENT;
        aligned_allocation.checked_add(PREIMAGE_CACHE_ENTRY_MEMORY_OVERHEAD)
    }

    /// Checks a cache miss against the transaction budget and then the block
    /// cap. The caller commits the returned totals only after insertion.
    fn next_estimated_byte_totals(
        &mut self,
        estimated_entry_bytes: usize,
        apply_transaction_budget: bool,
    ) -> Result<(usize, usize), SystemError> {
        let next_tx_bytes = if apply_transaction_budget {
            let Some(next_tx_bytes) = self
                .estimated_bytes_added_in_current_tx
                .checked_add(estimated_entry_bytes)
            else {
                self.tx_limit_hit_for_current_tx = true;
                return Err(out_of_native_resources!().into());
            };
            if next_tx_bytes > MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX {
                self.tx_limit_hit_for_current_tx = true;
                return Err(out_of_native_resources!().into());
            }
            next_tx_bytes
        } else {
            self.estimated_bytes_added_in_current_tx
        };

        let Some(next_retained_bytes) = self
            .estimated_retained_bytes
            .checked_add(estimated_entry_bytes)
        else {
            self.block_limit_hit_for_current_tx = true;
            return Err(out_of_native_resources!().into());
        };

        if next_retained_bytes > MAX_PREIMAGE_CACHE_RETAINED_BYTES {
            self.block_limit_hit_for_current_tx = true;
            return Err(out_of_native_resources!().into());
        }

        Ok((next_tx_bytes, next_retained_bytes))
    }

    /// Returns whether `additional_bytes` fit under the block-wide retained
    /// cache cap. This is used to reserve room for deferred account snapshots
    /// before a transaction is accepted.
    pub(super) fn can_retain_additional_bytes(&self, additional_bytes: usize) -> bool {
        self.estimated_retained_bytes
            .checked_add(additional_bytes)
            .is_some_and(|total| total <= MAX_PREIMAGE_CACHE_RETAINED_BYTES)
    }

    pub fn report_new_preimages(
        &self,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) -> Result<(), InternalError> {
        result_keeper.new_preimages(self.publication_storage.net_diffs_iter().map(|x| {
            let preimage = self
                .storage
                .get(x.key())
                .expect("preimage from publication storage must be known");

            (
                x.key(),
                preimage.as_slice(),
                x.current().value().preimage_type,
            )
        }));

        Ok(())
    }

    /// Charges decommitment work when a caller performs the cache lookup with
    /// separate, formally infinite resources to keep charging deterministic.
    pub fn charge_decommitment_native_cost(
        resources: &mut R,
        preimage_len: usize,
    ) -> Result<(), SystemError> {
        use zk_ee::system::Computational;

        let native_cost =
            PREIMAGE_CACHE_GET_NATIVE_COST.saturating_add(blake2s_native_cost(preimage_len));
        resources.charge(&R::from_native(R::Native::from_computational(native_cost)))
    }

    #[must_use]
    fn expose_preimage<const PROOF_ENV: bool>(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        preimage_type: PreimageType,
        hash: &Bytes32,
        expected_preimage_len_in_bytes: usize,
        resources: &mut R,
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError> {
        // Special case, for 0 hash we return an empty slice.
        if hash.is_zero() {
            return Ok(&[]);
        }

        // We charge the decommitment even if the hash is cached.
        // This way, native charging doesn't depend on the state of hashes.
        Self::charge_decommitment_native_cost(resources, expected_preimage_len_in_bytes)?;

        if let Some(cached) = self.storage.get(hash) {
            unsafe {
                let cached: &'static [u8] = core::mem::transmute(cached.as_slice());

                Ok(cached)
            }
        } else {
            let estimated_entry_bytes =
                match Self::estimated_entry_bytes(expected_preimage_len_in_bytes) {
                    Some(estimated_bytes) => estimated_bytes,
                    None => {
                        self.tx_limit_hit_for_current_tx = true;
                        return Err(out_of_native_resources!().into());
                    }
                };
            let (next_tx_bytes, next_retained_bytes) =
                self.next_estimated_byte_totals(estimated_entry_bytes, true)?;

            // We do not charge for gas in this concrete implementation and
            // expect higher-level model to do so.
            // We charge for native.
            let it = oracle
                .raw_query(FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID, hash)
                .expect("must make an iterator for preimage");
            // IMPORTANT: oracle should be somewhat "sane", it also limits the number of cycles spent below.

            if it.len() > num_usize_words_for_u8_capacity(expected_preimage_len_in_bytes) {
                return Err(
                    internal_error!("Iterator length exceeds expected preimage length").into(),
                );
            }
            let mut buffered =
                UsizeAlignedByteBox::from_usize_iterator_in(it, self.allocator.clone());
            // truncate
            buffered.truncated_to_byte_length(expected_preimage_len_in_bytes);

            if PROOF_ENV {
                match preimage_type {
                    PreimageType::AccountData => {
                        use crypto::blake2s::Blake2s256;
                        use crypto::MiniDigest;
                        let recomputed_hash =
                            Bytes32::from_array(Blake2s256::digest(buffered.as_slice()));

                        if recomputed_hash != *hash {
                            return Err(internal_error!("Account hash mismatch").into());
                        }
                    }
                    PreimageType::Bytecode => {
                        use crypto::blake2s::Blake2s256;
                        use crypto::MiniDigest;
                        let recomputed_hash =
                            Bytes32::from_array(Blake2s256::digest(buffered.as_slice()));

                        if recomputed_hash != *hash {
                            return Err(internal_error!("Bytecode hash mismatch").into());
                        }
                    }
                };
            } else {
                debug_assert!({
                    match preimage_type {
                        PreimageType::AccountData => {
                            use crypto::blake2s::Blake2s256;
                            use crypto::MiniDigest;
                            let recomputed_hash =
                                Bytes32::from_array(Blake2s256::digest(buffered.as_slice()));

                            recomputed_hash == *hash
                        }
                        PreimageType::Bytecode => {
                            use crypto::blake2s::Blake2s256;
                            use crypto::MiniDigest;
                            let recomputed_hash =
                                Bytes32::from_array(Blake2s256::digest(buffered.as_slice()));

                            recomputed_hash == *hash
                        }
                    }
                });
            }

            let inserted = self.storage.entry(*hash).or_insert(buffered);
            self.estimated_bytes_added_in_current_tx = next_tx_bytes;
            self.estimated_retained_bytes = next_retained_bytes;
            // Safety: IO implementer that will use it is expected to live beyond any frame (as it's part of the OS),
            // so we can extend the lifetime
            unsafe {
                let cached: &'static [u8] = core::mem::transmute(inserted.as_slice());

                Ok(cached)
            }
        }
    }

    /// Records an account snapshot during block finalization.
    ///
    /// Transactions have already been accepted at this point, so this skips
    /// the transaction-local budget. The block-wide retained-memory cap still
    /// applies, and room for these snapshots is checked before each candidate
    /// transaction is accepted.
    pub(super) fn record_preimage_for_block_finalization(
        &mut self,
        preimage_type: &PreimageRequest,
        resources: &mut R,
        preimage: &[&[u8]],
    ) -> Result<&'static [u8], SystemError> {
        self.record_preimage_inner(preimage_type, resources, preimage, false)
    }

    fn record_preimage_inner(
        &mut self,
        preimage_type: &PreimageRequest,
        resources: &mut R,
        preimage: &[&[u8]],
        apply_transaction_budget: bool,
    ) -> Result<&'static [u8], SystemError> {
        use crate::system_implementation::flat_storage_model::cost_constants::PREIMAGE_CACHE_SET_NATIVE_COST;
        use zk_ee::system::Computational;

        let PreimageRequest {
            hash,
            expected_preimage_len_in_bytes,
            preimage_type,
        } = preimage_type;

        let preimage_len = preimage.iter().try_fold(0usize, |acc, chunk| {
            acc.checked_add(chunk.len())
                .ok_or_else(|| internal_error!("Preimage length overflow"))
        })?;
        if preimage_len != *expected_preimage_len_in_bytes as usize {
            return Err(internal_error!("Unexpected preimage length").into());
        }

        let estimated_entry_bytes = match Self::estimated_entry_bytes(preimage_len) {
            Some(estimated_bytes) => estimated_bytes,
            None => {
                if apply_transaction_budget {
                    self.tx_limit_hit_for_current_tx = true;
                } else {
                    self.block_limit_hit_for_current_tx = true;
                }
                return Err(out_of_native_resources!().into());
            }
        };
        resources.charge(&R::from_native(R::Native::from_computational(
            PREIMAGE_CACHE_SET_NATIVE_COST,
        )))?;

        if self.storage.contains_key(hash) {
            self.publication_storage
                .add_preimage(hash, preimage_len, *preimage_type)?;
            let cached = self.storage.get(hash).expect("preimage was found above");
            // Safety: the cache is part of the OS and lives beyond every frame.
            return Ok(unsafe { core::mem::transmute::<&[u8], &'static [u8]>(cached.as_slice()) });
        }

        let (next_tx_bytes, next_retained_bytes) =
            self.next_estimated_byte_totals(estimated_entry_bytes, apply_transaction_budget)?;
        let boxed_data = UsizeAlignedByteBox::from_slices_in(preimage, self.allocator.clone());
        self.publication_storage
            .add_preimage(hash, preimage_len, *preimage_type)?;
        let inserted = self.storage.entry(*hash).or_insert(boxed_data);
        self.estimated_bytes_added_in_current_tx = next_tx_bytes;
        self.estimated_retained_bytes = next_retained_bytes;

        // Safety: the cache is part of the OS and lives beyond every frame.
        Ok(unsafe { core::mem::transmute::<&[u8], &'static [u8]>(inserted.as_slice()) })
    }
}

impl<R: Resources, A: Allocator + Clone> PreimageCacheModel
    for BytecodeAndAccountDataPreimagesStorage<R, A>
{
    type Resources = R;
    type PreimageRequest = PreimageRequest;

    fn get_preimage<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        preimage_type: &Self::PreimageRequest,
        resources: &mut Self::Resources,
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError> {
        // we will NOT charge ergs for preimages in here, but instead higher-level model should do it

        let PreimageRequest {
            hash,
            expected_preimage_len_in_bytes,
            preimage_type,
        } = preimage_type;

        // preimage type is not important in our case, we do not version them yet
        self.expose_preimage::<PROOF_ENV>(
            ee_type,
            *preimage_type,
            hash,
            *expected_preimage_len_in_bytes as usize,
            resources,
            oracle,
        )
    }

    fn record_preimage<const PROOF_ENV: bool>(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        preimage_type: &Self::PreimageRequest,
        resources: &mut Self::Resources,
        preimage: &[&[u8]],
    ) -> Result<&'static [u8], SystemError> {
        // we will NOT charge ergs for preimages in here, but instead higher-level model should do it
        self.record_preimage_inner(preimage_type, resources, preimage, true)
    }
}

impl<R: Resources, A: Allocator + Clone> SnapshottableIo
    for BytecodeAndAccountDataPreimagesStorage<R, A>
{
    type StateSnapshot = CacheSnapshotId;

    fn begin_new_tx(&mut self) {
        self.estimated_bytes_added_in_current_tx = 0;
        self.tx_limit_hit_for_current_tx = false;
        self.block_limit_hit_for_current_tx = false;
        self.publication_storage.begin_new_tx();
    }

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        self.publication_storage.finish_tx();
        Ok(())
    }

    fn start_frame(&mut self) -> Self::StateSnapshot {
        self.publication_storage.start_frame()
    }

    fn finish_frame(
        &mut self,
        rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError> {
        self.publication_storage.finish_frame(rollback_handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::{blake2s::Blake2s256, MiniDigest};
    use std::alloc::Global;
    use zk_ee::{
        common_structs::PreimageType,
        oracle::{
            usize_serialization::{UsizeDeserializable, UsizeSerializable},
            IOOracle,
        },
        reference_implementations::{BaseResources, DecreasingNative},
        system::Computational,
    };

    type TestResources = BaseResources<DecreasingNative>;
    type TestCache = BytecodeAndAccountDataPreimagesStorage<TestResources>;

    #[derive(Default)]
    struct TestOracle {
        words: Vec<usize>,
        queries: usize,
    }

    impl IOOracle for TestOracle {
        type RawIterator<'a> = std::vec::IntoIter<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            self.queries += 1;
            Ok(self.words.clone().into_iter())
        }
    }

    fn request_and_oracle() -> (PreimageRequest, TestOracle) {
        let word = 0x0102_0304usize;
        let bytes = word.to_ne_bytes();
        let hash = Bytes32::from_array(Blake2s256::digest(bytes));
        (
            PreimageRequest {
                hash,
                expected_preimage_len_in_bytes: bytes.len() as u32,
                preimage_type: PreimageType::Bytecode,
            },
            TestOracle {
                words: vec![word],
                queries: 0,
            },
        )
    }

    fn resources_with_native(native: u64) -> TestResources {
        TestResources::from_native(DecreasingNative::from_computational(native))
    }

    fn decommitment_native_cost(preimage_len: usize) -> u64 {
        PREIMAGE_CACHE_GET_NATIVE_COST + blake2s_native_cost(preimage_len)
    }

    fn record_native_cost() -> u64 {
        super::cost_constants::PREIMAGE_CACHE_SET_NATIVE_COST
    }

    #[test]
    fn cache_hit_and_miss_charge_the_same_native() {
        let (request, mut oracle) = request_and_oracle();
        let native_cost = decommitment_native_cost(request.expected_preimage_len_in_bytes as usize);
        let mut cache = TestCache::new_from_parts(Global);

        let mut miss_resources = resources_with_native(native_cost);
        cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut miss_resources,
                &mut oracle,
            )
            .unwrap();
        let retained_bytes_after_miss = cache.estimated_retained_bytes;

        let mut hit_resources = resources_with_native(native_cost);
        cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut hit_resources,
                &mut oracle,
            )
            .unwrap();

        assert_eq!(miss_resources.native().as_u64(), 0);
        assert_eq!(hit_resources.native().as_u64(), 0);
        assert_eq!(oracle.queries, 1);
        assert_eq!(cache.estimated_retained_bytes, retained_bytes_after_miss);
        assert_eq!(
            cache.estimated_bytes_added_in_current_tx,
            retained_bytes_after_miss
        );
    }

    #[test]
    fn insufficient_native_fails_before_query_or_retention() {
        let (request, mut oracle) = request_and_oracle();
        let native_cost = decommitment_native_cost(request.expected_preimage_len_in_bytes as usize);
        let mut resources = resources_with_native(native_cost - 1);
        let mut cache = TestCache::new_from_parts(Global);

        assert!(cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut resources,
                &mut oracle,
            )
            .is_err());
        assert_eq!(oracle.queries, 0);
        assert_eq!(cache.estimated_retained_bytes, 0);
        assert_eq!(cache.estimated_bytes_added_in_current_tx, 0);
        assert!(cache.storage.is_empty());
        assert!(!cache.tx_limit_hit_for_current_tx());
        assert!(!cache.block_limit_hit_for_current_tx());
    }

    #[test]
    fn block_cap_fails_before_query_and_sets_only_block_flag() {
        let (request, mut oracle) = request_and_oracle();
        let preimage_len = request.expected_preimage_len_in_bytes as usize;
        let estimated_entry_bytes = TestCache::estimated_entry_bytes(preimage_len).unwrap();
        let initial_retained_bytes = MAX_PREIMAGE_CACHE_RETAINED_BYTES - estimated_entry_bytes + 1;
        let mut cache = TestCache::new_from_parts(Global);
        cache.estimated_retained_bytes = initial_retained_bytes;
        let mut resources = resources_with_native(decommitment_native_cost(preimage_len));

        assert!(cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut resources,
                &mut oracle,
            )
            .is_err());
        assert_eq!(oracle.queries, 0);
        assert!(cache.storage.is_empty());
        assert_eq!(cache.estimated_retained_bytes, initial_retained_bytes);
        assert_eq!(cache.estimated_bytes_added_in_current_tx, 0);
        assert!(!cache.tx_limit_hit_for_current_tx());
        assert!(cache.block_limit_hit_for_current_tx());

        cache.begin_new_tx();
        assert!(!cache.tx_limit_hit_for_current_tx());
        assert!(!cache.block_limit_hit_for_current_tx());
        assert_eq!(cache.estimated_retained_bytes, initial_retained_bytes);
    }

    #[test]
    fn tx_budget_fails_before_query_and_resets_at_next_tx() {
        let (request, mut oracle) = request_and_oracle();
        let preimage_len = request.expected_preimage_len_in_bytes as usize;
        let estimated_entry_bytes = TestCache::estimated_entry_bytes(preimage_len).unwrap();
        let initial_tx_bytes = MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX - estimated_entry_bytes + 1;
        let mut cache = TestCache::new_from_parts(Global);
        cache.estimated_bytes_added_in_current_tx = initial_tx_bytes;
        let mut resources = resources_with_native(decommitment_native_cost(preimage_len));

        assert!(cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut resources,
                &mut oracle,
            )
            .is_err());
        assert_eq!(oracle.queries, 0);
        assert!(cache.storage.is_empty());
        assert_eq!(cache.estimated_bytes_added_in_current_tx, initial_tx_bytes);
        assert_eq!(cache.estimated_retained_bytes, 0);
        assert!(cache.tx_limit_hit_for_current_tx());
        assert!(!cache.block_limit_hit_for_current_tx());

        cache.begin_new_tx();
        assert_eq!(cache.estimated_bytes_added_in_current_tx, 0);
        assert_eq!(cache.estimated_retained_bytes, 0);
        assert!(!cache.tx_limit_hit_for_current_tx());
        assert!(!cache.block_limit_hit_for_current_tx());
    }

    #[test]
    fn duplicate_record_and_frame_rollback_preserve_retained_byte_accounting() {
        let preimage = [1u8, 2, 3, 4, 5];
        let request = PreimageRequest {
            hash: Bytes32::from_array(Blake2s256::digest(preimage)),
            expected_preimage_len_in_bytes: preimage.len() as u32,
            preimage_type: PreimageType::Bytecode,
        };
        let native_cost = record_native_cost();
        let mut cache = TestCache::new_from_parts(Global);
        cache.begin_new_tx();
        let rollback_handle = cache.start_frame();

        let mut first_resources = resources_with_native(native_cost);
        cache
            .record_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut first_resources,
                &[&preimage],
            )
            .unwrap();
        let retained_bytes_after_first = cache.estimated_retained_bytes;
        let tx_bytes_after_first = cache.estimated_bytes_added_in_current_tx;

        let mut duplicate_resources = resources_with_native(native_cost);
        cache
            .record_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut duplicate_resources,
                &[&preimage],
            )
            .unwrap();
        assert_eq!(cache.storage.len(), 1);
        assert_eq!(cache.estimated_retained_bytes, retained_bytes_after_first);
        assert_eq!(
            cache.estimated_bytes_added_in_current_tx,
            tx_bytes_after_first
        );

        cache.finish_frame(Some(&rollback_handle)).unwrap();
        assert_eq!(cache.storage.len(), 1);
        assert_eq!(cache.estimated_retained_bytes, retained_bytes_after_first);
        assert_eq!(
            cache.estimated_bytes_added_in_current_tx,
            tx_bytes_after_first
        );
    }

    #[test]
    fn record_checks_tx_budget_before_inserting() {
        let preimage = [9u8; 16];
        let request = PreimageRequest {
            hash: Bytes32::from_array(Blake2s256::digest(preimage)),
            expected_preimage_len_in_bytes: preimage.len() as u32,
            preimage_type: PreimageType::Bytecode,
        };
        let estimated_entry_bytes = TestCache::estimated_entry_bytes(preimage.len()).unwrap();
        let initial_tx_bytes = MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX - estimated_entry_bytes + 1;
        let mut cache = TestCache::new_from_parts(Global);
        cache.estimated_bytes_added_in_current_tx = initial_tx_bytes;
        let mut resources = resources_with_native(record_native_cost());

        assert!(cache
            .record_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut resources,
                &[&preimage],
            )
            .is_err());
        assert!(cache.storage.is_empty());
        assert_eq!(cache.estimated_retained_bytes, 0);
        assert_eq!(cache.estimated_bytes_added_in_current_tx, initial_tx_bytes);
        assert!(cache.tx_limit_hit_for_current_tx());
        assert!(!cache.block_limit_hit_for_current_tx());
    }

    #[test]
    fn block_finalization_skips_tx_budget_but_still_checks_block_cap() {
        let preimage = [7u8; 16];
        let request = PreimageRequest {
            hash: Bytes32::from_array(Blake2s256::digest(preimage)),
            expected_preimage_len_in_bytes: preimage.len() as u32,
            preimage_type: PreimageType::AccountData,
        };
        let estimated_entry_bytes = TestCache::estimated_entry_bytes(preimage.len()).unwrap();
        let mut cache = TestCache::new_from_parts(Global);
        cache.estimated_bytes_added_in_current_tx = MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX;
        let mut resources = resources_with_native(record_native_cost());

        cache
            .record_preimage_for_block_finalization(&request, &mut resources, &[&preimage])
            .unwrap();
        assert_eq!(cache.estimated_retained_bytes, estimated_entry_bytes);
        assert_eq!(
            cache.estimated_bytes_added_in_current_tx,
            MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX
        );
        assert!(!cache.tx_limit_hit_for_current_tx());

        let second_preimage = [8u8; 16];
        let second_request = PreimageRequest {
            hash: Bytes32::from_array(Blake2s256::digest(second_preimage)),
            expected_preimage_len_in_bytes: second_preimage.len() as u32,
            preimage_type: PreimageType::AccountData,
        };
        cache.estimated_retained_bytes =
            MAX_PREIMAGE_CACHE_RETAINED_BYTES - estimated_entry_bytes + 1;
        let mut second_resources = resources_with_native(record_native_cost());
        assert!(cache
            .record_preimage_for_block_finalization(
                &second_request,
                &mut second_resources,
                &[&second_preimage],
            )
            .is_err());
        assert!(!cache.tx_limit_hit_for_current_tx());
        assert!(cache.block_limit_hit_for_current_tx());
    }

    #[test]
    fn tx_budget_is_half_the_block_cap() {
        assert_eq!(
            MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX * 2,
            MAX_PREIMAGE_CACHE_RETAINED_BYTES
        );
    }
}
