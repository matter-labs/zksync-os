//! Storage slot cache of the Ethereum storage model, backed by a history map
//! keyed by interned indices: the address index and the slot key index packed
//! into one `u32` (see [`slot_cache_key`]), searched in a hash table. The
//! cached slots of every account are also chained together, so the slots of
//! one account can be walked without an ordered index: that serves the
//! storage clean-up of a self-destructed account and the grouping of the
//! state commitment.
use core::alloc::Allocator;
use storage_models::common_structs::snapshottable_io::SnapshottableIo;
use zk_ee::common_structs::history_counter::NonEmptyHistoryCounter;
#[cfg(feature = "slot_cache_btree_index")]
use zk_ee::common_structs::history_map::BTreeIndex;
#[cfg(not(feature = "slot_cache_btree_index"))]
use zk_ee::common_structs::history_map::HashIndex;
use zk_ee::common_structs::history_map::{ElementHandle, HistoryMap};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::basic_queries::InitialStorageSlotQuery;
use zk_ee::oracle::memory_io::OracleQuery;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::{
    system::{errors::system::SystemError, Resources},
    types_config::EthereumIOTypesConfig,
    utils::Bytes32,
};

use crate::system_implementation::caches::cache_element_state::{
    ObservedValue, TransactionId, Warmth,
};
use crate::system_implementation::caches::generic_pubdata_aware_plain_storage::{
    element_values, ElementValues, IsWarmRead, StorageElementRecord, StorageSnapshotId,
};
use crate::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use crate::system_implementation::ethereum_storage_model::interner::{
    slot_cache_key, split_slot_cache_key, Interned, MAX_UNIQUE_ADDRESSES, MAX_UNIQUE_SLOTS,
};
use ruint::aliases::B160;

pub type SlotRecord = StorageElementRecord<Bytes32>;

/// Per-element link of the chain of cached slots of one account
pub struct SlotChainLink<A: Allocator + Clone> {
    next: Option<SlotHandle<A>>,
}

pub type SlotHandle<A> = ElementHandle<u32, SlotRecord, A, SlotChainLink<A>>;

/// How the slot cache searches its keys: a hash table, or an ordered tree over
/// the same packed `u32` keys for comparison (`slot_cache_btree_index`).
#[cfg(not(feature = "slot_cache_btree_index"))]
pub type SlotIndex<A> = HashIndex<SlotHandle<A>, A>;
#[cfg(feature = "slot_cache_btree_index")]
pub type SlotIndex<A> = BTreeIndex<u32, SlotHandle<A>, A>;

fn new_slot_index<A: Allocator + Clone>(alloc: A) -> SlotIndex<A> {
    #[cfg(not(feature = "slot_cache_btree_index"))]
    {
        HashIndex::new_in(MAX_UNIQUE_SLOTS, alloc)
    }
    #[cfg(feature = "slot_cache_btree_index")]
    {
        let _ = MAX_UNIQUE_SLOTS;
        BTreeIndex::new_in(alloc)
    }
}

pub type SlotCacheMap<A> = HistoryMap<u32, SlotRecord, A, SlotChainLink<A>, SlotIndex<A>>;

type SlotItem<'a, A> = zk_ee::common_structs::history_map::HistoryMapItemRefMut<
    'a,
    u32,
    SlotRecord,
    A,
    SlotChainLink<A>,
>;

pub struct EthereumStorageCache<
    A: Allocator + Clone,
    SF: StackFactory<N>,
    const N: usize,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
> {
    pub(crate) cache: SlotCacheMap<A>,
    /// Head of the chain of cached slots of every account, by address index.
    /// Room for every address is reserved once; the length grows as accounts
    /// get slots.
    slots_by_account: alloc::vec::Vec<Option<SlotHandle<A>>, A>,
    pub(crate) resources_policy: P,
    // Note: this doesn't need to be equal to the actual tx number in the block, it just needs to be able to differentiate between transactions.
    pub(crate) current_tx_id: TransactionId,
    pub(crate) evm_refunds_counter: NonEmptyHistoryCounter<R, SF, N, A>, // Used to keep track of EVM gas refunds
    pub(crate) alloc: A,
    pub(crate) _marker: core::marker::PhantomData<(R, SF)>,
}

impl<
        A: Allocator + Clone,
        SF: StackFactory<N>,
        const N: usize,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
    > SnapshottableIo for EthereumStorageCache<A, SF, N, R, P>
{
    type StateSnapshot = StorageSnapshotId;

    fn begin_new_tx(&mut self) {
        self.cache.commit();
        self.evm_refunds_counter =
            NonEmptyHistoryCounter::new_with_initial(self.alloc.clone(), R::empty());
        // Advance the warmth id at the start of each tx (not at finish) so that
        // block-level system operations, which run before the first `begin_new_tx`,
        // keep tx id 0 and are never considered warm by user transactions
        // (which start at id 1). Matches the account cache, which bumps on begin.
        //
        // This is also what makes every element cold again: warmth is bound to the
        // transaction that established it. Elements that were only touched stay in
        // the cache as cold and unobserved; they are inert (no charge ever depends
        // on their presence, see `materialize_element`) and are skipped when the
        // state changes are reported.
        self.current_tx_id.0 += 1;
    }

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        Ok(())
    }

    fn start_frame(&mut self) -> Self::StateSnapshot {
        StorageSnapshotId {
            cache: self.cache.snapshot(),
            evm_refunds_counter: self.evm_refunds_counter.snapshot(),
        }
    }

    fn finish_frame(
        &mut self,
        rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError> {
        if let Some(x) = rollback_handle {
            self.evm_refunds_counter.rollback(x.evm_refunds_counter);
            self.cache.rollback(x.cache)
        } else {
            Ok(())
        }
    }
}

impl<
        A: Allocator + Clone,
        SF: StackFactory<N>,
        const N: usize,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
    > EthereumStorageCache<A, SF, N, R, P>
{
    pub fn new_from_parts(allocator: A, resources_policy: P) -> Self {
        let slots_by_account =
            alloc::vec::Vec::with_capacity_in(MAX_UNIQUE_ADDRESSES, allocator.clone());
        Self {
            cache: HistoryMap::with_index(new_slot_index(allocator.clone()), allocator.clone()),
            slots_by_account,
            current_tx_id: TransactionId(0),
            resources_policy,
            evm_refunds_counter: NonEmptyHistoryCounter::new_with_initial(
                allocator.clone(),
                R::empty(),
            ),
            alloc: allocator,
            _marker: core::marker::PhantomData,
        }
    }

    #[track_caller]
    pub fn finish_frame_impl(
        &mut self,
        rollback_handle: Option<&StorageSnapshotId>,
    ) -> Result<(), InternalError> {
        self.finish_frame(rollback_handle)
    }

    /// Reads the block-start value of the slot from the oracle
    fn read_initial_value(
        oracle: &mut impl IOOracle,
        address: &B160,
        key: &Bytes32,
    ) -> Result<ObservedValue<Bytes32>, SystemError> {
        // the oracle speaks big-endian; the cache may hold little-endian keys and values
        const LE: bool = crate::system_implementation::ethereum_storage_model::STORAGE_SLOTS_LE;
        let mut query_key = *key;
        if LE {
            query_key.bytereverse();
        }
        let data_from_oracle =
            InitialStorageSlotQuery::<EthereumIOTypesConfig>::get(oracle, (address, &query_key))
                .map_err(|_| internal_error!("Must get initial slot value from oracle"))?;
        let mut value = data_from_oracle.initial_value;
        if LE {
            value.bytereverse();
        }

        // We need to check that the initial value is default
        if data_from_oracle.is_new_storage_slot {
            assert_eq!(
                Bytes32::ZERO,
                value,
                "Initial value of empty slot must be trivial"
            );
        }

        Ok(ObservedValue::Observed {
            value,
            is_new: data_from_oracle.is_new_storage_slot,
        })
    }

    /// Links a just inserted element into the chain of its account's slots
    #[inline(always)]
    fn link_new_slot(
        slots_by_account: &mut alloc::vec::Vec<Option<SlotHandle<A>>, A>,
        address_index: u32,
        item: &mut SlotItem<'_, A>,
    ) {
        let address_index = address_index as usize;
        if address_index >= slots_by_account.len() {
            // within the reserved capacity (the interner bounds the index): no reallocation
            slots_by_account.resize(address_index + 1, None);
        }
        let head = &mut slots_by_account[address_index];
        item.element_properties_mut().next = *head;
        *head = Some(item.handle());
    }

    /// Warms an element up without reading it, as an access list or a precompile
    /// warm-up does. No oracle IO happens, so only the warm read is charged: an
    /// element that is only touched creates no proof obligation, and the native
    /// (merkle) part of the cold cost is paid by the first actual read or write.
    pub fn touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        key: Interned<'_, Bytes32>,
        _oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        // Only warms the slot up. Its value is not read, so no merkle proof
        // obligation is created: Ethereum does not load access-list slots either,
        // and a witness legitimately lacks proofs for slots the block never reads.
        self.resources_policy
            .charge_warm_storage_read(ee_type, resources)?;

        let current_tx_id = self.current_tx_id;
        let mut inserted = false;
        let mut item = self.cache.get_or_insert_checked::<_, SystemError>(
            &slot_cache_key(address.index, key.index),
            &mut inserted,
            |_, _| Ok(()),
            |inserted| {
                *inserted = true;
                Ok((
                    StorageElementRecord::new(ObservedValue::Unobserved),
                    SlotChainLink { next: None },
                ))
            },
        )?;
        if inserted {
            Self::link_new_slot(&mut self.slots_by_account, address.index, &mut item);
        }
        if !item.current().warmth.is_warm(current_tx_id) {
            item.update(|record| {
                record.warmth = Warmth::Warm {
                    in_tx: current_tx_id,
                };
                Ok::<_, SystemError>(())
            })?;
        }

        Ok(())
    }

    /// Read element and initialize it if needed
    #[allow(clippy::too_many_arguments)]
    fn materialize_element<'a>(
        cache: &'a mut SlotCacheMap<A>,
        slots_by_account: &mut alloc::vec::Vec<Option<SlotHandle<A>>, A>,
        resources_policy: &mut P,
        current_tx_id: TransactionId,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        key: Interned<'_, Bytes32>,
        oracle: &mut impl IOOracle,
    ) -> Result<(SlotItem<'a, A>, IsWarmRead), SystemError> {
        resources_policy.charge_warm_storage_read(ee_type, resources)?;

        // Conservative pre-gate for a cold read: the slot IO runs unmetered and the
        // real cold charge only happens at warm-up, so without this a tx could
        // force that (prover) work and then fail the charge, leaving it unpaid. So
        // we check up front that the worst-case cold read is affordable. Charging
        // a throwaway copy of the resources here is just a way to check we have
        // enough — nothing is spent, the real charge still happens at warm-up. A
        // new slot is the costliest cold read (an extra non-inclusion merkle
        // path), so it upper-bounds the existing-slot case and the real warm-up
        // charge below cannot fail. The bound is data-independent and warmth is
        // rollback-aware, so the gate is identical across sequencer and proving
        // and never depends on cache-entry presence.
        //
        // The gate runs inside the single map search: on a found element only if
        // it is cold or was never read, and before the IO for a missing one. A
        // warm element that was only touched pays no gas for the read, so the
        // probe mirrors that, or it would refuse a read the EVM allows.
        fn probe_cold_read<R: Resources, P: StorageAccessPolicy<R, Bytes32>>(
            resources_policy: &mut P,
            resources: &R,
            ee_type: ExecutionEnvironmentType,
            is_warm_access: bool,
        ) -> Result<(), SystemError> {
            let mut probe = resources.clone();
            resources_policy.charge_cold_storage_read_extra(
                ee_type,
                &mut probe,
                true,
                is_warm_access,
            )
        }
        let mut context = (resources_policy, resources, oracle, false);

        let mut item = cache.get_or_insert_checked(
            &slot_cache_key(address.index, key.index),
            &mut context,
            |(resources_policy, resources, _, _), item| {
                let record = item.current();
                let is_warm = record.warmth.is_warm(current_tx_id);
                if !is_warm || record.value.is_unobserved() {
                    probe_cold_read::<R, P>(resources_policy, resources, ee_type, is_warm)?;
                }
                Ok::<_, SystemError>(())
            },
            |(resources_policy, resources, oracle, inserted)| {
                probe_cold_read::<R, P>(resources_policy, resources, ee_type, false)?;
                // Element doesn't exist in cache yet, initialize it.
                // Cold access charging happens at warm-up below: the initial
                // record persists even if the inserting transaction is dropped
                // from the block, so anything charging-related must live in
                // rollback-aware metadata.
                let value = Self::read_initial_value(*oracle, address.key, key.key)?;
                *inserted = true;

                // Note: we initialize it as cold, should be warmed up separately
                // Since in case of revert it should become cold again and initial record can't be rolled back
                Ok((
                    StorageElementRecord::new(value),
                    SlotChainLink { next: None },
                ))
            },
        )?;
        let (resources_policy, resources, oracle, inserted) = context;
        if inserted {
            Self::link_new_slot(slots_by_account, address.index, &mut item);
        }

        // An element that was only touched is observed on its first read. Observation
        // is not a state change but a fact about the whole history (no write can have
        // happened before it), so it is filled into every record in place: a rollback
        // of the frame that read it keeps the value and only reverts the warmth.
        let newly_observed = item.current().value.is_unobserved();
        if newly_observed {
            let value = Self::read_initial_value(oracle, address.key, key.key)?;
            item.for_each_record_mut(|record| record.value = value);
        }

        let record = item.current();
        let is_warm_read = record.warmth.is_warm(current_tx_id);
        let ObservedValue::Observed { is_new, .. } = record.value else {
            return Err(internal_error!("materialized storage element must be observed").into());
        };
        let new_read_extra_charged = record.new_read_extra_charged;

        // The cold extra has a gas part for a cold access (EIP-2929) and a native
        // part for the merkle work of the first read. A warm element that was only
        // touched still has the latter ahead and pays the native part alone.
        if !is_warm_read || newly_observed {
            // The NEW read extra (tree non-inclusion check) is charged once
            // per slot per block; later cold accesses pay EXISTING. "Already
            // paid" is tracked in metadata so that it rolls back together
            // with the paying transaction if it's dropped from the block.
            let charge_as_new = is_new && !new_read_extra_charged;
            resources_policy.charge_cold_storage_read_extra(
                ee_type,
                resources,
                charge_as_new,
                is_warm_read,
            )?;

            item.update(|record| {
                record.warmth = Warmth::Warm {
                    in_tx: current_tx_id,
                };
                if charge_as_new {
                    record.new_read_extra_charged = true;
                }
                Ok::<_, SystemError>(())
            })?;
        }

        Ok((item, IsWarmRead(is_warm_read)))
    }

    pub fn read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        key: Interned<'_, Bytes32>,
        oracle: &mut impl IOOracle,
    ) -> Result<Bytes32, SystemError> {
        let mut out = None;
        self.read_and_place(ee_type, resources, address, key, oracle, |value| {
            out = Some(*value)
        })?;
        // `place` runs exactly once on success
        out.ok_or_else(|| internal_error!("storage read placed no value").into())
    }

    /// Reads the element and hands its current value to `place` by reference, so the
    /// caller copies it once, straight into its destination.
    pub fn read_and_place(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        key: Interned<'_, Bytes32>,
        oracle: &mut impl IOOracle,
        place: impl FnOnce(&Bytes32),
    ) -> Result<(), SystemError> {
        let (addr_data, _) = Self::materialize_element(
            &mut self.cache,
            &mut self.slots_by_account,
            &mut self.resources_policy,
            self.current_tx_id,
            ee_type,
            resources,
            address,
            key,
            oracle,
        )?;

        match &addr_data.current().value {
            ObservedValue::Observed { value, .. } => {
                place(value);
                Ok(())
            }
            ObservedValue::Unobserved => {
                Err(internal_error!("materialized storage element must be observed").into())
            }
        }
    }

    pub fn write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        key: Interned<'_, Bytes32>,
        new_value: &Bytes32,
        oracle: &mut impl IOOracle,
    ) -> Result<Bytes32, SystemError> {
        let (mut addr_data, is_warm_read) = Self::materialize_element(
            &mut self.cache,
            &mut self.slots_by_account,
            &mut self.resources_policy,
            self.current_tx_id,
            ee_type,
            resources,
            address,
            key,
            oracle,
        )?;

        let (val_at_tx_start, val_current, is_new) =
            match (&addr_data.committed().value, &addr_data.current().value) {
                (
                    ObservedValue::Observed {
                        value: val_at_tx_start,
                        ..
                    },
                    ObservedValue::Observed {
                        value: val_current,
                        is_new,
                    },
                ) => (val_at_tx_start, val_current, *is_new),
                (ObservedValue::Unobserved, _) | (_, ObservedValue::Unobserved) => {
                    return Err(
                        internal_error!("materialized storage element must be observed").into(),
                    );
                }
            };
        // Use NEW write-extra only for the first cold write to a truly new slot.
        // Once the insertion cost has been paid, subsequent txs pay EXISTING.
        let is_new_slot = is_new && addr_data.current().write_extra_charged_in_tx.is_none();

        // Two separate warmness flags:
        // - is_warm_access: EIP-2929 access warmness (any prior SLOAD or SSTORE) — for ergs
        // - is_cold_write_charged: cold write extra already paid this tx — for native
        let is_warm_access = is_warm_read.0;
        let is_cold_write_charged =
            addr_data.current().write_extra_charged_in_tx == Some(self.current_tx_id);

        self.resources_policy.charge_storage_write_extra(
            ee_type,
            val_at_tx_start,
            val_current,
            new_value,
            resources,
            is_warm_access,
            is_cold_write_charged,
            is_new_slot,
        )?;

        // Compute refund before mutating the cache, so val_at_tx_start and
        // val_current can stay borrowed from addr_data.
        let mut refund_counter_value = self.evm_refunds_counter.value().clone();
        self.resources_policy.refund_for_storage_write(
            ee_type,
            val_at_tx_start,
            val_current,
            new_value,
            resources,
            &mut refund_counter_value,
        )?;

        // Detach owned old_value from addr_data's borrow before the update.
        let old_value = *val_current;
        let current_tx_id = self.current_tx_id;

        addr_data.update(|record| {
            record.value = ObservedValue::Observed {
                value: *new_value,
                is_new,
            };
            if !is_cold_write_charged && new_value != &old_value {
                record.write_extra_charged_in_tx = Some(current_tx_id);
            }
            Ok::<_, SystemError>(())
        })?;
        self.evm_refunds_counter.update(refund_counter_value);

        Ok(old_value)
    }

    /// Clears every cached slot of the account with this address index
    pub fn clear_state_impl(&mut self, address_index: u32) -> Result<(), SystemError> {
        let mut next = self.head_of_account(address_index);
        while let Some(handle) = next {
            // Safety: the handle came from this map through `link_new_slot`, and
            // the map is never cleared.
            let mut item = unsafe { self.cache.item_mut(handle) };
            next = item.element_properties().next;
            // An element that was only touched stays unknown: there is nothing
            // to clear, and clearing it would create a proof obligation for a
            // value nobody read.
            if item.current().value.is_unobserved() {
                continue;
            }
            item.update(|record| {
                if let ObservedValue::Observed { value, .. } = &mut record.value {
                    *value = Bytes32::ZERO;
                }
                Ok::<_, InternalError>(())
            })?;
        }

        Ok(())
    }

    /// First element of the chain of cached slots of an account, if it has any
    #[inline(always)]
    fn head_of_account(&self, address_index: u32) -> Option<SlotHandle<A>> {
        self.slots_by_account
            .get(address_index as usize)
            .copied()
            .flatten()
    }

    pub fn get_refund_counter_impl(&'_ self) -> &'_ R {
        self.evm_refunds_counter.value()
    }

    pub fn add_to_refund_counter_impl(&mut self, refund: R) -> Result<(), SystemError> {
        let mut t = self.get_refund_counter_impl().clone();
        t.add_ergs(refund.ergs());
        self.evm_refunds_counter.update(t);
        Ok(())
    }

    /// Number of accessed storage slots (across all the accounts)
    pub fn num_accesses(&self) -> usize {
        self.cache.len()
    }

    /// The block-level view of every cached slot, as `((address index, slot key
    /// index), values)`, in the order of the index
    pub fn iter_slots(
        &self,
    ) -> impl ExactSizeIterator<Item = ((u32, u32), ElementValues<Bytes32>)> + Clone + '_ {
        self.cache
            .iter()
            .map(|item| (split_slot_cache_key(*item.key()), element_values(&item)))
    }

    /// The block-level view of the cached slots of one account, as `(slot key
    /// index, values)`, most recently cached first
    pub fn iter_slots_of_account(
        &self,
        address_index: u32,
    ) -> impl Iterator<Item = (u32, ElementValues<Bytes32>)> + '_ {
        let mut next = self.head_of_account(address_index);
        core::iter::from_fn(move || {
            let handle = next?;
            // Safety: the handle came from this map through `link_new_slot`, and
            // the map is never cleared.
            let item = unsafe { self.cache.item(handle) };
            next = item.key_properties().next;
            Some((split_slot_cache_key(*item.key()).1, element_values(&item)))
        })
    }

    /// The block-level view of one cached slot
    pub fn get_slot(&self, address_index: u32, key_index: u32) -> Option<ElementValues<Bytes32>> {
        self.cache
            .get(&slot_cache_key(address_index, key_index))
            .map(|item| element_values(&item))
    }

    pub fn calculate_pubdata_used_by_tx(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_implementation::ethereum_storage_model::interner::EthereumKeyInterners;
    use crate::system_implementation::system::EthereumLikeStorageAccessCostModel;
    use std::alloc::Global;
    use std::collections::BTreeMap;
    use zk_ee::common_structs::WarmStorageKey;
    use zk_ee::memory::stack_implementations::vec_stack::VecStackFactory;
    use zk_ee::oracle::memory_io::host::{
        NativeQuerierMemory, ReadQueryInput, ResponseBuffer, WriteQueryOutput,
    };
    use zk_ee::oracle::memory_io::MemoryOracle;
    use zk_ee::oracle::query_ids::INITIAL_STORAGE_SLOT_VALUE_QUERY_ID;
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
    use zk_ee::storage_types::InitialStorageSlotData;
    use zk_ee::system::Resource;

    type TestResources = BaseResources<DecreasingNative>;
    type TestCache = EthereumStorageCache<
        Global,
        VecStackFactory,
        4,
        TestResources,
        EthereumLikeStorageAccessCostModel,
    >;

    /// Answers slot queries from a fixed map (missing slots are new) and counts them
    struct CountingOracle {
        existing: BTreeMap<(WarmStorageKey, ()), Bytes32>,
        queries: usize,
        response: ResponseBuffer,
    }

    impl MemoryOracle for CountingOracle {
        fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), InternalError> {
            assert_eq!(query_id, INITIAL_STORAGE_SLOT_VALUE_QUERY_ID);
            self.queries += 1;
            // SAFETY: the test sends the queries from this process
            let memory = unsafe { NativeQuerierMemory::new() };
            let (address, key) = <(B160, Bytes32)>::read_input(&memory, input_word)?;
            let queried = WarmStorageKey { address, key };
            let slot_data = match self.existing.get(&(queried, ())) {
                Some(value) => InitialStorageSlotData::<EthereumIOTypesConfig> {
                    is_new_storage_slot: false,
                    initial_value: *value,
                },
                None => InitialStorageSlotData::<EthereumIOTypesConfig> {
                    is_new_storage_slot: true,
                    initial_value: Bytes32::ZERO,
                },
            };
            let mut response = vec![];
            slot_data.write_output(&mut response);
            self.response.set(response)
        }

        zk_ee::memory_oracle_response_methods!(response);
    }

    impl IOOracle for CountingOracle {
        type RawIterator<'a> = core::iter::Empty<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            panic!("unexpected query {query_type:#x}")
        }
    }

    const A: B160 = B160::from_limbs([0x1234, 0, 0]);
    const B: B160 = B160::from_limbs([0x5678, 0, 0]);
    fn key(byte: u8) -> Bytes32 {
        Bytes32::from_byte_fill(byte)
    }
    fn slot_of(address: B160, key_byte: u8) -> WarmStorageKey {
        WarmStorageKey {
            address,
            key: key(key_byte),
        }
    }

    struct Fixture {
        cache: TestCache,
        interners: EthereumKeyInterners<Global>,
        oracle: CountingOracle,
    }

    fn setup() -> Fixture {
        let mut cache = TestCache::new_from_parts(Global, EthereumLikeStorageAccessCostModel);
        cache.begin_new_tx();
        let oracle = CountingOracle {
            existing: BTreeMap::from([
                ((slot_of(A, 2), ()), key(0xaa)),
                ((slot_of(B, 2), ()), key(0xbb)),
                ((slot_of(B, 3), ()), key(0xcc)),
            ]),
            queries: 0,
            response: ResponseBuffer::default(),
        };
        Fixture {
            cache,
            interners: EthereumKeyInterners::new_in(Global),
            oracle,
        }
    }

    impl Fixture {
        fn slot(
            &mut self,
            address: &'static B160,
            key_byte: u8,
        ) -> (Interned<'static, B160>, Interned<'static, Bytes32>) {
            let key: &'static Bytes32 = Box::leak(Box::new(key(key_byte)));
            (
                self.interners.intern_address(address).unwrap(),
                self.interners.intern_slot_key(key).unwrap(),
            )
        }

        fn touch(&mut self, address: &'static B160, key_byte: u8) {
            let (a, k) = self.slot(address, key_byte);
            let mut resources = TestResources::FORMAL_INFINITE;
            self.cache
                .touch(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    a,
                    k,
                    &mut self.oracle,
                )
                .expect("touch");
        }

        fn read(&mut self, address: &'static B160, key_byte: u8) -> Bytes32 {
            let (a, k) = self.slot(address, key_byte);
            let mut resources = TestResources::FORMAL_INFINITE;
            self.cache
                .read(
                    ExecutionEnvironmentType::EVM,
                    &mut resources,
                    a,
                    k,
                    &mut self.oracle,
                )
                .expect("read")
        }

        fn write(&mut self, address: &'static B160, key_byte: u8, value: Bytes32) -> Bytes32 {
            let (a, k) = self.slot(address, key_byte);
            let mut resources = TestResources::FORMAL_INFINITE;
            self.cache
                .write(
                    ExecutionEnvironmentType::EVM,
                    &mut resources,
                    a,
                    k,
                    &value,
                    &mut self.oracle,
                )
                .expect("write")
        }

        /// The cached slots of an account as (slot key, initial, current, observed), by key
        fn slots_of(&self, address: &B160) -> Vec<(Bytes32, Bytes32, Bytes32, bool)> {
            let index = self.interners.address_index(address).expect("interned");
            let mut slots: Vec<_> = self
                .cache
                .iter_slots_of_account(index)
                .map(|(key_index, values)| {
                    (
                        self.interners.slot_keys()[key_index as usize],
                        values.initial,
                        values.current,
                        values.is_observed,
                    )
                })
                .collect();
            slots.sort();
            slots
        }
    }

    #[test]
    fn slots_are_chained_per_account_and_shared_keys_stay_apart() {
        let mut f = setup();
        assert_eq!(f.read(&A, 2), key(0xaa));
        assert_eq!(f.read(&B, 2), key(0xbb), "same slot key, other account");
        f.touch(&A, 1);
        assert_eq!(f.write(&B, 3, key(0x11)), key(0xcc));
        assert_eq!(f.write(&A, 4, key(0x22)), Bytes32::ZERO);
        assert_eq!(f.oracle.queries, 4, "the touched slot is not read");
        assert_eq!(f.cache.num_accesses(), 5);
        assert_eq!(f.interners.slot_keys.len(), 4, "keys 1..=4");
        assert_eq!(f.interners.addresses.len(), 2);

        assert_eq!(
            f.slots_of(&A),
            vec![
                (key(1), Bytes32::ZERO, Bytes32::ZERO, false),
                (key(2), key(0xaa), key(0xaa), true),
                (key(4), Bytes32::ZERO, key(0x22), true),
            ]
        );
        assert_eq!(
            f.slots_of(&B),
            vec![
                (key(2), key(0xbb), key(0xbb), true),
                (key(3), key(0xcc), key(0x11), true),
            ]
        );
        let a = f.interners.address_index(&A).unwrap();
        let k4 = f.interners.slot_key_index(&key(4)).unwrap();
        assert_eq!(f.cache.get_slot(a, k4).unwrap().current, key(0x22));
        let b = f.interners.address_index(&B).unwrap();
        assert!(f.cache.get_slot(b, k4).is_none(), "B never used key 4");
        assert_eq!(f.cache.iter_slots().len(), 5);
        assert!(
            f.cache.iter_slots_of_account(a + 100).next().is_none(),
            "unknown account has no slots"
        );
    }

    #[test]
    fn clearing_an_account_zeroes_its_observed_slots_only() {
        let mut f = setup();
        f.read(&A, 2);
        f.touch(&A, 1);
        f.write(&A, 4, key(0x22));
        f.write(&B, 3, key(0x11));
        f.read(&B, 2);

        let a = f.interners.address_index(&A).unwrap();
        f.cache.clear_state_impl(a).expect("clear");

        assert_eq!(
            f.slots_of(&A),
            vec![
                (key(1), Bytes32::ZERO, Bytes32::ZERO, false),
                (key(2), key(0xaa), Bytes32::ZERO, true),
                (key(4), Bytes32::ZERO, Bytes32::ZERO, true),
            ],
            "observed slots are zeroed, the touched one stays unknown"
        );
        assert_eq!(
            f.slots_of(&B),
            vec![
                (key(2), key(0xbb), key(0xbb), true),
                (key(3), key(0xcc), key(0x11), true),
            ],
            "the other account is untouched"
        );
        assert_eq!(
            f.read(&A, 2),
            Bytes32::ZERO,
            "the clearing is visible to reads"
        );
        assert_eq!(f.oracle.queries, 4, "no slot is read again");
        // an account that never had slots
        f.cache.clear_state_impl(a + 100).expect("nothing to clear");
    }
}
