use alloc::{alloc::Global, collections::BTreeMap};
use core::{alloc::Allocator, marker::PhantomData};
use evm_interpreter::BitMapOwned;
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

/// On the 32-bit proving target, `UsizeAlignedByteBox` rounds allocations to
/// pairs of four-byte native words.
const PREIMAGE_CACHE_ALLOCATION_ALIGNMENT: usize = 8;
/// Conservative allowance for the key, value, B-tree node, and allocator
/// metadata retained by each raw cache entry.
const PREIMAGE_CACHE_ENTRY_MEMORY_OVERHEAD: usize = 256;
/// Transaction indices in logs are `u16`. Cache candidate IDs deliberately
/// use the same fixed domain; `begin_new_tx` rejects any attempt to exceed it.
const MAX_PREIMAGE_CACHE_TX_IDS: usize = u16::MAX as usize + 1;
/// The raw preimage cache may retain at most 256 MiB, including conservative
/// per-entry map and allocator overhead.
pub const MAX_PREIMAGE_CACHE_RETAINED_BYTES: usize = 256 * 1024 * 1024;
/// A single transaction may add at most half of the block cache limit.
pub const MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX: usize = MAX_PREIMAGE_CACHE_RETAINED_BYTES / 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct PreimageRequest {
    pub hash: Bytes32,
    pub expected_preimage_len_in_bytes: u32,
    pub preimage_type: PreimageType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreimageAdmission {
    /// The entry was introduced by a transaction known to be included in the
    /// block, or by block finalization.
    Accepted,
    /// The entry was first retained while this candidate transaction was
    /// executing. Its acceptance is resolved lazily through the bitmap.
    Pending(u16),
}

struct CachedPreimage<A: Allocator> {
    bytes: UsizeAlignedByteBox<A>,
    admission: PreimageAdmission,
}

impl<A: Allocator> CachedPreimage<A> {
    fn as_slice(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// Block-scoped cache whose raw entries survive transaction and frame rollback.
///
/// The block budget includes physical entries and conservative precharges for
/// final account snapshots. Physical cache hits do not consume it. They bypass
/// the transaction budget only when the entry was introduced by an accepted
/// transaction or was already counted for the current candidate. Before a
/// physical miss is allocated, the cache checks both budgets.
///
/// Exceeding the transaction budget returns ordinary OON. Exceeding the block
/// cap sets `block_limit_hit_for_current_tx`; the bootloader then rolls the
/// transaction back as `BlockNativeLimitReached` without committing effects or
/// fees. `begin_new_tx` resets transaction-local accounting and flags, but the
/// raw entries and block-retained-or-reserved byte counter deliberately remain.
pub struct BytecodeAndAccountDataPreimagesStorage<R: Resources, A: Allocator + Clone = Global> {
    storage: BTreeMap<Bytes32, CachedPreimage<A>, A>,
    pub(crate) publication_storage: NewPreimagesPublicationStorage<A>,
    pub(crate) allocator: A,
    accepted_transactions: BitMapOwned<A>,
    /// ID of the candidate currently executing, or `None` outside candidate
    /// execution and after the fixed `u16` ID space is exhausted.
    current_tx_id: Option<u16>,
    /// ID to assign to the next candidate. `None` means all `u16` IDs have
    /// already been used.
    next_tx_id: Option<u16>,
    estimated_retained_bytes: usize,
    estimated_bytes_added_in_current_tx: usize,
    tx_limit_hit_for_current_tx: bool,
    block_limit_hit_for_current_tx: bool,
    _marker: PhantomData<R>,
}

impl<R: Resources, A: Allocator + Clone> BytecodeAndAccountDataPreimagesStorage<R, A> {
    pub fn new_from_parts(allocator: A) -> Self {
        let publication_storage = NewPreimagesPublicationStorage::new_from_parts(allocator.clone());
        let accepted_transactions =
            BitMapOwned::allocate_for_bit_capacity(MAX_PREIMAGE_CACHE_TX_IDS, allocator.clone());
        Self {
            storage: BTreeMap::new_in(allocator.clone()),
            publication_storage,
            allocator,
            accepted_transactions,
            current_tx_id: None,
            next_tx_id: Some(0),
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

    #[cfg(test)]
    pub(super) fn estimated_retained_bytes(&self) -> usize {
        self.estimated_retained_bytes
    }

    #[cfg(test)]
    pub(super) fn estimated_bytes_added_in_current_tx(&self) -> usize {
        self.estimated_bytes_added_in_current_tx
    }

    #[cfg(test)]
    pub(super) fn set_estimated_bytes_added_in_current_tx(&mut self, bytes: usize) {
        self.estimated_bytes_added_in_current_tx = bytes;
    }

    /// Computes the transaction-local total after charging one logically new
    /// cache entry. On overflow or limit exhaustion it sets the transaction
    /// flag and returns OON; otherwise the caller commits the returned total
    /// only after admission or insertion succeeds.
    fn next_estimated_tx_bytes(
        &mut self,
        estimated_entry_bytes: usize,
    ) -> Result<usize, SystemError> {
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

        Ok(next_tx_bytes)
    }

    /// Checks a physical cache miss against the transaction budget and then
    /// the block cap. The caller commits the returned totals only after a
    /// successful insertion.
    fn next_estimated_byte_totals(
        &mut self,
        estimated_entry_bytes: usize,
        apply_transaction_budget: bool,
    ) -> Result<(usize, usize), SystemError> {
        let next_tx_bytes = if apply_transaction_budget {
            self.next_estimated_tx_bytes(estimated_entry_bytes)?
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

    /// Returns the ID of the candidate that may own a `Pending` entry.
    fn active_candidate_id(&self) -> Result<u16, SystemError> {
        match self.current_tx_id {
            Some(tx_id) => Ok(tx_id),
            None if self.block_limit_hit_for_current_tx => Err(out_of_native_resources!().into()),
            None => Err(
                internal_error!("pending preimage admission requires an active candidate").into(),
            ),
        }
    }

    /// Chooses admission for a physical miss. Work outside candidate execution
    /// is common to sequencing and proving and is accepted immediately.
    fn admission_for_new_preimage(&self) -> Result<PreimageAdmission, SystemError> {
        match self.current_tx_id {
            Some(tx_id) => Ok(PreimageAdmission::Pending(tx_id)),
            None if self.block_limit_hit_for_current_tx => Err(out_of_native_resources!().into()),
            None => Ok(PreimageAdmission::Accepted),
        }
    }

    /// Makes a physical cache hit logically visible to the current candidate.
    /// Entries introduced by invalidated candidates are charged again, while
    /// entries introduced by accepted candidates are lazily canonicalized.
    fn admit_cached_preimage_with_admission_for_current_tx(
        &mut self,
        hash: &Bytes32,
        admission: PreimageAdmission,
        preimage_len: usize,
    ) -> Result<(), SystemError> {
        match admission {
            // Block-level work or a previous lookup already established that
            // this entry belongs to the accepted block state.
            PreimageAdmission::Accepted => Ok(()),
            // The entry is still tagged with its creator, but that candidate
            // was accepted. Canonicalize it lazily; the current candidate does
            // not pay the transaction-local cache charge again.
            PreimageAdmission::Pending(tx_id)
                if self
                    .accepted_transactions
                    .get_bit(tx_id as usize)
                    .unwrap_or(false) =>
            {
                self.storage
                    .get_mut(hash)
                    .expect("cached preimage must remain present")
                    .admission = PreimageAdmission::Accepted;
                Ok(())
            }
            PreimageAdmission::Pending(tx_id) => {
                let current_tx_id = self.active_candidate_id()?;
                // The current candidate created or already re-admitted this
                // entry, so it has already paid the transaction-local charge.
                if tx_id == current_tx_id {
                    return Ok(());
                }
                // The creator was not accepted and is not the current
                // candidate. Its bytes remain physically cached, but the entry
                // is logically new here: charge it and transfer Pending
                // ownership to the current candidate.
                let estimated_entry_bytes = match Self::estimated_entry_bytes(preimage_len) {
                    Some(estimated_bytes) => estimated_bytes,
                    None => {
                        self.tx_limit_hit_for_current_tx = true;
                        return Err(out_of_native_resources!().into());
                    }
                };
                let next_tx_bytes = self.next_estimated_tx_bytes(estimated_entry_bytes)?;
                self.storage
                    .get_mut(hash)
                    .expect("cached preimage must remain present")
                    .admission = PreimageAdmission::Pending(current_tx_id);
                self.estimated_bytes_added_in_current_tx = next_tx_bytes;
                Ok(())
            }
        }
    }

    /// Re-admits a preimage whose decoded value was served by a higher-level
    /// cache instead of through `get_preimage`.
    ///
    /// Account-cache entries survive a dropped candidate. On a later cold
    /// account access, the decoded properties can therefore be reused without
    /// another preimage-cache lookup. Admission must still be resolved here so
    /// a preimage introduced only by the dropped candidate consumes the later
    /// candidate's transaction-local budget, just as it does in proving.
    pub(super) fn admit_cached_preimage_for_current_tx(
        &mut self,
        hash: &Bytes32,
    ) -> Result<(), SystemError> {
        let Some(cached) = self.storage.get(hash) else {
            return Err(internal_error!("Materialized preimage is missing from cache").into());
        };
        let admission = cached.admission;
        // Charge the size of the entry that is physically retained.
        let preimage_len = cached.as_slice().len();

        self.admit_cached_preimage_with_admission_for_current_tx(hash, admission, preimage_len)
    }

    /// Conservatively precharges one cache entry that may be materialized
    /// during block finalization. Reservations deliberately survive rollback,
    /// just like raw cache entries and account-cache materialization.
    pub(super) fn reserve_preimage_for_block_finalization(
        &mut self,
        preimage_len: usize,
    ) -> Result<(), SystemError> {
        let estimated_entry_bytes = match Self::estimated_entry_bytes(preimage_len) {
            Some(estimated_bytes) => estimated_bytes,
            None => {
                self.block_limit_hit_for_current_tx = true;
                return Err(out_of_native_resources!().into());
            }
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

        self.estimated_retained_bytes = next_retained_bytes;
        Ok(())
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
            let admission = cached.admission;
            // Safety: the backing allocation is owned by the block-scoped
            // cache and entries are never removed.
            let cached = unsafe { core::mem::transmute::<&[u8], &'static [u8]>(cached.as_slice()) };
            self.admit_cached_preimage_with_admission_for_current_tx(
                hash,
                admission,
                expected_preimage_len_in_bytes,
            )?;
            Ok(cached)
        } else {
            let admission = self.admission_for_new_preimage()?;
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

            let inserted = self.storage.entry(*hash).or_insert(CachedPreimage {
                bytes: buffered,
                admission,
            });
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

    /// Records an account snapshot during block finalization. Its retained
    /// memory was conservatively precharged when the account first entered the
    /// account cache, so insertion must not charge either budget again.
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

        if let Some(cached) = self.storage.get(hash) {
            let admission = cached.admission;
            // Safety: the backing allocation is owned by the block-scoped
            // cache and entries are never removed.
            let cached = unsafe { core::mem::transmute::<&[u8], &'static [u8]>(cached.as_slice()) };
            if apply_transaction_budget {
                self.admit_cached_preimage_with_admission_for_current_tx(
                    hash,
                    admission,
                    preimage_len,
                )?;
            } else if admission != PreimageAdmission::Accepted {
                self.storage
                    .get_mut(hash)
                    .expect("cached preimage must remain present")
                    .admission = PreimageAdmission::Accepted;
            }
            self.publication_storage
                .add_preimage(hash, preimage_len, *preimage_type)?;
            return Ok(cached);
        }

        let admission = if apply_transaction_budget {
            PreimageAdmission::Pending(self.active_candidate_id()?)
        } else {
            PreimageAdmission::Accepted
        };
        let (next_tx_bytes, next_retained_bytes) = if apply_transaction_budget {
            self.next_estimated_byte_totals(estimated_entry_bytes, true)?
        } else {
            // Block finalization inserts a new entry, but
            // `reserve_preimage_for_block_finalization` already counted its
            // bytes when the account entered the account cache. Counting them
            // here would charge every updated account twice.
            (
                self.estimated_bytes_added_in_current_tx,
                self.estimated_retained_bytes,
            )
        };
        let boxed_data = UsizeAlignedByteBox::from_slices_in(preimage, self.allocator.clone());
        self.publication_storage
            .add_preimage(hash, preimage_len, *preimage_type)?;
        let inserted = self.storage.entry(*hash).or_insert(CachedPreimage {
            bytes: boxed_data,
            admission,
        });
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
        self.current_tx_id = self.next_tx_id;
        if let Some(tx_id) = self.current_tx_id {
            self.next_tx_id = tx_id.checked_add(1);
            self.block_limit_hit_for_current_tx = false;
        } else {
            // All `u16` candidate IDs were used. Do not wrap and alias an
            // earlier candidate's acceptance bit.
            self.block_limit_hit_for_current_tx = true;
        }
        self.publication_storage.begin_new_tx();
    }

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        let tx_id = self.current_tx_id.take().ok_or_else(|| {
            internal_error!("cannot accept a transaction without a valid u16 candidate ID")
        })?;
        if !self.accepted_transactions.set_bit_on(tx_id as usize) {
            return Err(internal_error!(
                "transaction ID is outside the acceptance bitmap"
            ));
        }
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
            TestCache::estimated_entry_bytes(request.expected_preimage_len_in_bytes as usize)
                .unwrap()
        );
    }

    #[test]
    fn cache_entry_from_invalidated_tx_is_charged_again() {
        let (request, mut oracle) = request_and_oracle();
        let preimage_len = request.expected_preimage_len_in_bytes as usize;
        let native_cost = decommitment_native_cost(preimage_len);
        let estimated_entry_bytes = TestCache::estimated_entry_bytes(preimage_len).unwrap();
        let mut cache = TestCache::new_from_parts(Global);

        cache.begin_new_tx();
        let rollback_handle = cache.start_frame();
        let mut first_resources = resources_with_native(native_cost);
        cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut first_resources,
                &mut oracle,
            )
            .unwrap();
        cache.finish_frame(Some(&rollback_handle)).unwrap();
        let retained_bytes = cache.estimated_retained_bytes;

        // Starting the next candidate without finishing the previous one
        // leaves transaction 0 invalidated.
        cache.begin_new_tx();
        let mut second_resources = resources_with_native(native_cost);
        cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut second_resources,
                &mut oracle,
            )
            .unwrap();

        assert_eq!(oracle.queries, 1);
        assert_eq!(cache.estimated_retained_bytes, retained_bytes);
        assert_eq!(
            cache.estimated_bytes_added_in_current_tx,
            estimated_entry_bytes
        );
        assert_eq!(
            cache.storage.get(&request.hash).unwrap().admission,
            PreimageAdmission::Pending(1)
        );
    }

    #[test]
    fn block_level_entry_is_accepted_before_first_candidate() {
        let (request, mut oracle) = request_and_oracle();
        let preimage_len = request.expected_preimage_len_in_bytes as usize;
        let native_cost = decommitment_native_cost(preimage_len);
        let mut cache = TestCache::new_from_parts(Global);

        let mut block_resources = resources_with_native(native_cost);
        cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::NoEE,
                &request,
                &mut block_resources,
                &mut oracle,
            )
            .unwrap();
        assert_eq!(
            cache.storage.get(&request.hash).unwrap().admission,
            PreimageAdmission::Accepted
        );

        // Candidate 0 is invalidated without touching the entry. Candidate 1
        // must still see the common block-level entry as accepted.
        cache.begin_new_tx();
        cache.begin_new_tx();
        let mut tx_resources = resources_with_native(native_cost);
        cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut tx_resources,
                &mut oracle,
            )
            .unwrap();

        assert_eq!(oracle.queries, 1);
        assert_eq!(cache.estimated_bytes_added_in_current_tx, 0);
    }

    #[test]
    fn cache_entry_from_accepted_tx_is_lazily_promoted() {
        let (request, mut oracle) = request_and_oracle();
        let preimage_len = request.expected_preimage_len_in_bytes as usize;
        let native_cost = decommitment_native_cost(preimage_len);
        let mut cache = TestCache::new_from_parts(Global);

        cache.begin_new_tx();
        let rollback_handle = cache.start_frame();
        let mut first_resources = resources_with_native(native_cost);
        cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut first_resources,
                &mut oracle,
            )
            .unwrap();
        // Raw preimages and their pending admission survive internal frame
        // rollback. Accepting the transaction resolves the pending tx ID.
        cache.finish_frame(Some(&rollback_handle)).unwrap();
        cache.finish_tx().unwrap();

        cache.begin_new_tx();
        let mut second_resources = resources_with_native(native_cost);
        cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut second_resources,
                &mut oracle,
            )
            .unwrap();

        assert_eq!(oracle.queries, 1);
        assert_eq!(cache.estimated_bytes_added_in_current_tx, 0);
        assert_eq!(
            cache.storage.get(&request.hash).unwrap().admission,
            PreimageAdmission::Accepted
        );
    }

    #[test]
    fn transaction_id_limit_does_not_wrap() {
        let (request, mut oracle) = request_and_oracle();
        let preimage_len = request.expected_preimage_len_in_bytes as usize;
        let native_cost = decommitment_native_cost(preimage_len);
        let mut cache = TestCache::new_from_parts(Global);
        cache.next_tx_id = Some(u16::MAX);

        cache.begin_new_tx();
        assert_eq!(cache.current_tx_id, Some(u16::MAX));
        assert_eq!(cache.next_tx_id, None);
        assert!(!cache.block_limit_hit_for_current_tx());
        cache.finish_tx().unwrap();

        cache.begin_new_tx();
        assert_eq!(cache.current_tx_id, None);
        assert!(cache.block_limit_hit_for_current_tx());
        let mut resources = resources_with_native(native_cost);
        assert!(cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &request,
                &mut resources,
                &mut oracle,
            )
            .is_err());
        assert_eq!(oracle.queries, 0);
        assert!(cache.finish_tx().is_err());
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
        cache.begin_new_tx();
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
    fn block_finalization_uses_precharged_budget() {
        let preimage = [7u8; 16];
        let request = PreimageRequest {
            hash: Bytes32::from_array(Blake2s256::digest(preimage)),
            expected_preimage_len_in_bytes: preimage.len() as u32,
            preimage_type: PreimageType::AccountData,
        };
        let estimated_entry_bytes = TestCache::estimated_entry_bytes(preimage.len()).unwrap();
        let mut cache = TestCache::new_from_parts(Global);
        cache.estimated_bytes_added_in_current_tx = MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX;
        cache.estimated_retained_bytes = MAX_PREIMAGE_CACHE_RETAINED_BYTES - estimated_entry_bytes;
        cache
            .reserve_preimage_for_block_finalization(preimage.len())
            .unwrap();
        let mut resources = resources_with_native(record_native_cost());

        cache
            .record_preimage_for_block_finalization(&request, &mut resources, &[&preimage])
            .unwrap();
        assert_eq!(
            cache.estimated_retained_bytes,
            MAX_PREIMAGE_CACHE_RETAINED_BYTES
        );
        assert_eq!(
            cache.estimated_bytes_added_in_current_tx,
            MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX
        );
        assert!(!cache.tx_limit_hit_for_current_tx());
        assert!(!cache.block_limit_hit_for_current_tx());
    }

    #[test]
    fn precharge_prevents_candidate_from_consuming_finalization_space() {
        let final_preimage = [7u8; 16];
        let final_request = PreimageRequest {
            hash: Bytes32::from_array(Blake2s256::digest(final_preimage)),
            expected_preimage_len_in_bytes: final_preimage.len() as u32,
            preimage_type: PreimageType::AccountData,
        };
        let estimated_entry_bytes = TestCache::estimated_entry_bytes(final_preimage.len()).unwrap();
        let mut cache = TestCache::new_from_parts(Global);
        cache.estimated_retained_bytes = MAX_PREIMAGE_CACHE_RETAINED_BYTES - estimated_entry_bytes;
        cache
            .reserve_preimage_for_block_finalization(final_preimage.len())
            .unwrap();

        cache.begin_new_tx();
        let (candidate_request, mut oracle) = request_and_oracle();
        let candidate_len = candidate_request.expected_preimage_len_in_bytes as usize;
        let mut candidate_resources =
            resources_with_native(decommitment_native_cost(candidate_len));
        assert!(cache
            .get_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &candidate_request,
                &mut candidate_resources,
                &mut oracle,
            )
            .is_err());
        assert!(cache.block_limit_hit_for_current_tx());

        let mut finalization_resources = resources_with_native(record_native_cost());
        cache
            .record_preimage_for_block_finalization(
                &final_request,
                &mut finalization_resources,
                &[&final_preimage],
            )
            .unwrap();
        assert_eq!(
            cache.estimated_retained_bytes,
            MAX_PREIMAGE_CACHE_RETAINED_BYTES
        );
    }

    #[test]
    fn finalization_precharge_survives_frame_rollback() {
        let preimage_len = 16;
        let estimated_entry_bytes = TestCache::estimated_entry_bytes(preimage_len).unwrap();
        let mut cache = TestCache::new_from_parts(Global);
        cache.begin_new_tx();
        let rollback_handle = cache.start_frame();

        cache
            .reserve_preimage_for_block_finalization(preimage_len)
            .unwrap();
        cache.finish_frame(Some(&rollback_handle)).unwrap();

        assert_eq!(cache.estimated_retained_bytes, estimated_entry_bytes);
    }
}
