//! Storage cache, backed by a history map.
use crate::system_implementation::caches::cache_element_state::{ObservedValue, Warmth};
use crate::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use alloc::fmt::Debug;
use core::alloc::Allocator;
use ruint::aliases::B160;
use zk_ee::common_structs::history_counter::HistoryCounterSnapshotId;
use zk_ee::common_structs::history_counter::NonEmptyHistoryCounter;
use zk_ee::common_traits::key_like_with_bounds::{KeyLikeWithBounds, TyEq};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::oracle::basic_queries::InitialStorageSlotQuery;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::{
    memory::stack_trait::StackFactory,
    oracle::simple_oracle_query::SimpleOracleQuery,
    storage_types::StorageAddress,
    system::{errors::system::SystemError, Resources},
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
};

use zk_ee::common_structs::history_map::*;

pub use crate::system_implementation::caches::cache_element_state::TransactionId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsWarmRead(pub bool);

type AddressItem<'a, K, V, A> = HistoryMapItemRefMut<'a, K, StorageElementRecord<V>, A>;

/// One history record of a cached storage element: the two rollback-aware facts
/// about it (warmth, value knowledge) packed with the charging markers.
///
/// The `value` of the head record is the current value, of the committed record
/// the value at the start of the transaction, and of the initial record the
/// block-start value. All of them are `Unobserved` until the element is read for
/// the first time, which fills the block-start value into every record in place
/// (nothing else can have changed it before), so a rollback can never make an
/// observed element unobserved again.
#[derive(Clone, Debug)]
pub struct StorageElementRecord<V> {
    /// EIP-2929 warmth, established by any read, write or touch, and reverted
    /// with the frame that established it.
    pub warmth: Warmth,
    /// What is known about the value. A touch leaves it `Unobserved`: no oracle
    /// IO happened and no proof obligation exists.
    pub value: ObservedValue<V>,
    /// Transaction where cold write extra was last charged for this slot.
    /// Used to distinguish "warm because previously written" (write paths paid)
    /// from "warm because previously read" (only read paths paid).
    pub write_extra_charged_in_tx: Option<TransactionId>,
    /// Whether the cold NEW-slot read extra was already charged for this slot.
    /// Kept in rollback-aware metadata rather than derived from cache presence:
    /// entries materialized by a transaction that is later dropped from the
    /// block stay in the cache, but their metadata updates are rolled back
    /// together with the charge, so charging never depends on dropped
    /// transactions (which the proving run doesn't re-execute).
    pub new_read_extra_charged: bool,
}

impl<V> StorageElementRecord<V> {
    /// A cold record with no charges recorded yet
    pub fn new(value: ObservedValue<V>) -> Self {
        Self {
            warmth: Warmth::Cold,
            value,
            write_extra_charged_in_tx: None,
            new_read_extra_charged: false,
        }
    }
}

/// Block-level view of one cached element for diff and proof reporting
#[derive(Clone, Copy, Debug)]
pub struct ElementValues<V> {
    pub initial: V,
    pub current: V,
    pub is_new: bool,
    /// `false` for an element that was only touched: nothing is known about it
    /// and it creates no proof obligation. The other fields are then defaults.
    pub is_observed: bool,
}

/// Summarizes an element's history for reporting
pub fn element_values<K, V: Default + Clone, A: Allocator + Clone>(
    item: &HistoryMapItemRef<'_, K, StorageElementRecord<V>, A>,
) -> ElementValues<V> {
    match (&item.initial().value, &item.current().value) {
        (
            ObservedValue::Observed {
                value: initial,
                is_new,
            },
            ObservedValue::Observed { value: current, .. },
        ) => ElementValues {
            initial: initial.clone(),
            current: current.clone(),
            is_new: *is_new,
            is_observed: true,
        },
        // observation fills the whole history, so the records agree
        (ObservedValue::Unobserved, _) | (_, ObservedValue::Unobserved) => ElementValues {
            initial: V::default(),
            current: V::default(),
            is_new: false,
            is_observed: false,
        },
    }
}

#[derive(Debug)]
pub struct StorageSnapshotId {
    pub cache: CacheSnapshotId,
    pub evm_refunds_counter: HistoryCounterSnapshotId,
}

pub struct GenericPubdataAwarePlainStorage<
    K: KeyLikeWithBounds,
    V,
    A: Allocator + Clone, // = Global,
    SF: StackFactory<M>,
    const M: usize,
    R: Resources,
    P: StorageAccessPolicy<R, V>,
> {
    pub(crate) cache: HistoryMap<K, StorageElementRecord<V>, A>,
    pub(crate) resources_policy: P,
    // Note: this doesn't need to be equal to the actual tx number in the block, it just needs to be able to differentiate between transactions.
    pub(crate) current_tx_id: TransactionId,
    pub(crate) evm_refunds_counter: NonEmptyHistoryCounter<R, SF, M, A>, // Used to keep track of EVM gas refunds
    pub(crate) alloc: A,
    pub(crate) _marker: core::marker::PhantomData<(R, SF)>,
}

impl<
        K: 'static + KeyLikeWithBounds,
        V: Default
            + Clone
            + Debug
            + PartialEq
            + From<<EthereumIOTypesConfig as SystemIOTypesConfig>::StorageValue>,
        A: Allocator + Clone,
        SF: StackFactory<M>,
        const M: usize,
        R: Resources,
        P: StorageAccessPolicy<R, V>,
    > GenericPubdataAwarePlainStorage<K, V, A, SF, M, R, P>
{
    pub fn new_from_parts(allocator: A, resources_policy: P) -> Self {
        Self {
            cache: HistoryMap::new(allocator.clone()),
            current_tx_id: TransactionId(0),
            resources_policy,
            evm_refunds_counter: NonEmptyHistoryCounter::new_with_initial(
                allocator.clone(),
                R::empty(),
            ),
            alloc: allocator.clone(),
            _marker: core::marker::PhantomData,
        }
    }

    pub fn begin_new_tx(&mut self) {
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

    pub fn finish_tx(&mut self) {}

    #[track_caller]
    pub fn start_frame(&mut self) -> StorageSnapshotId {
        StorageSnapshotId {
            cache: self.cache.snapshot(),
            evm_refunds_counter: self.evm_refunds_counter.snapshot(),
        }
    }

    #[track_caller]
    #[must_use]
    pub fn finish_frame_impl(
        &mut self,
        rollback_handle: Option<&StorageSnapshotId>,
    ) -> Result<(), InternalError> {
        if let Some(x) = rollback_handle {
            self.evm_refunds_counter.rollback(x.evm_refunds_counter);
            self.cache.rollback(x.cache)
        } else {
            Ok(())
        }
    }

    /// Reads the block-start value of `key` from the oracle
    fn read_initial_value(
        oracle: &mut impl IOOracle,
        key: &K,
    ) -> Result<ObservedValue<V>, SystemError>
    where
        StorageAddress<EthereumIOTypesConfig>: From<K>,
    {
        let query_input = (*key).into();
        let data_from_oracle = InitialStorageSlotQuery::get(oracle, &query_input)
            .map_err(|_| internal_error!("Must get initial slot value from oracle"))?;
        let value: V = data_from_oracle.initial_value.into();

        // We need to check that the initial value is default
        if data_from_oracle.is_new_storage_slot {
            assert_eq!(
                V::default(),
                value,
                "Initial value of empty slot must be trivial"
            );
        }

        Ok(ObservedValue::Observed {
            value,
            is_new: data_from_oracle.is_new_storage_slot,
        })
    }

    /// Warms an element up without reading it, as an access list or a precompile
    /// warm-up does. No oracle IO happens, so only the warm read is charged: an
    /// element that is only touched creates no proof obligation, and the native
    /// (merkle) part of the cold cost is paid by the first actual read or write.
    pub fn apply_touch_impl(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        key: &K,
        resources: &mut R,
    ) -> Result<(), SystemError> {
        self.resources_policy
            .charge_warm_storage_read(ee_type, resources)?;

        let current_tx_id = self.current_tx_id;
        let mut item = self.cache.get_or_insert::<SystemError>(key, || {
            Ok((StorageElementRecord::new(ObservedValue::Unobserved), ()))
        })?;
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
    fn materialize_element<'a>(
        cache: &'a mut HistoryMap<K, StorageElementRecord<V>, A>,
        resources_policy: &mut P,
        current_tx_id: TransactionId,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        key: &'a K,
        oracle: &mut impl IOOracle,
    ) -> Result<(AddressItem<'a, K, V, A>, IsWarmRead), SystemError>
    where
        StorageAddress<EthereumIOTypesConfig>: From<K>,
    {
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
        fn probe_cold_read<R: Resources, V, P: StorageAccessPolicy<R, V>>(
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
        let mut context = (resources_policy, resources, oracle);

        let mut item = cache.get_or_insert_checked(
            key,
            &mut context,
            |(resources_policy, resources, _), item| {
                let record = item.current();
                let is_warm = record.warmth.is_warm(current_tx_id);
                if !is_warm || record.value.is_unobserved() {
                    probe_cold_read::<R, V, P>(resources_policy, resources, ee_type, is_warm)?;
                }
                Ok::<_, SystemError>(())
            },
            |(resources_policy, resources, oracle)| {
                probe_cold_read::<R, V, P>(resources_policy, resources, ee_type, false)?;
                // Element doesn't exist in cache yet, initialize it.
                // Cold access charging happens at warm-up below: the initial
                // record persists even if the inserting transaction is dropped
                // from the block, so anything charging-related must live in
                // rollback-aware metadata.
                let value = Self::read_initial_value(*oracle, key)?;

                // Note: we initialize it as cold, should be warmed up separately
                // Since in case of revert it should become cold again and initial record can't be rolled back
                Ok((StorageElementRecord::new(value), ()))
            },
        )?;
        let (resources_policy, resources, oracle) = context;

        // An element that was only touched is observed on its first read. Observation
        // is not a state change but a fact about the whole history (no write can have
        // happened before it), so it is filled into every record in place: a rollback
        // of the frame that read it keeps the value and only reverts the warmth.
        let newly_observed = item.current().value.is_unobserved();
        if newly_observed {
            let value = Self::read_initial_value(oracle, key)?;
            item.for_each_record_mut(|record| record.value = value.clone());
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

    pub fn apply_read_impl(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        key: &K,
        resources: &mut R,
        oracle: &mut impl IOOracle,
    ) -> Result<V, SystemError>
    where
        StorageAddress<EthereumIOTypesConfig>: From<K>,
    {
        let mut out = None;
        self.apply_read_impl_with(ee_type, key, resources, oracle, |value| {
            out = Some(value.clone())
        })?;
        // `place` runs exactly once on success
        out.ok_or_else(|| internal_error!("storage read placed no value").into())
    }

    /// Reads the element and hands its current value to `place` by reference, so the
    /// caller copies it once, straight into its destination.
    pub fn apply_read_impl_with(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        key: &K,
        resources: &mut R,
        oracle: &mut impl IOOracle,
        place: impl FnOnce(&V),
    ) -> Result<(), SystemError>
    where
        StorageAddress<EthereumIOTypesConfig>: From<K>,
    {
        let (addr_data, _) = Self::materialize_element(
            &mut self.cache,
            &mut self.resources_policy,
            self.current_tx_id,
            ee_type,
            resources,
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

    pub fn apply_write_impl(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        key: &K,
        new_value: &V,
        oracle: &mut impl IOOracle,
        resources: &mut R,
    ) -> Result<V, SystemError>
    where
        StorageAddress<EthereumIOTypesConfig>: From<K>,
    {
        let (mut addr_data, is_warm_read) = Self::materialize_element(
            &mut self.cache,
            &mut self.resources_policy,
            self.current_tx_id,
            ee_type,
            resources,
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
        let old_value = val_current.clone();
        let current_tx_id = self.current_tx_id;

        addr_data.update(|record| {
            record.value = ObservedValue::Observed {
                value: new_value.clone(),
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

    /// Clear state at specified address
    pub fn clear_state_impl(&mut self, address: impl AsRef<B160>) -> Result<(), SystemError>
    where
        K::Subspace: TyEq<B160>,
    {
        use core::ops::Bound::Included;
        let lower_bound = K::lower_bound(TyEq::rwi(*address.as_ref()));
        let upper_bound = K::upper_bound(TyEq::rwi(*address.as_ref()));
        self.cache
            .for_each_range((Included(&lower_bound), Included(&upper_bound)), |mut x| {
                // An element that was only touched stays unknown: there is nothing
                // to clear, and clearing it would create a proof obligation for a
                // value nobody read.
                if x.current().value.is_unobserved() {
                    return Ok(());
                }
                x.update(|record| {
                    if let ObservedValue::Observed { value, .. } = &mut record.value {
                        *value = V::default();
                    }
                    Ok(())
                })
            })?;

        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_implementation::flat_storage_model::cost_constants::{
        COLD_EXISTING_STORAGE_READ_NATIVE_COST, COLD_NEW_STORAGE_READ_NATIVE_COST,
        WARM_STORAGE_READ_NATIVE_COST, WARM_STORAGE_WRITE_EXTRA_NATIVE_COST,
    };
    use crate::system_implementation::system::EthereumLikeStorageAccessCostModel;
    use evm_interpreter::gas_constants::{
        COLD_SLOAD_COST, SSTORE_SET_EXTRA, WARM_STORAGE_READ_COST,
    };
    use std::alloc::Global;
    use std::collections::BTreeMap;
    use zk_ee::common_structs::WarmStorageKey;
    use zk_ee::memory::stack_implementations::vec_stack::VecStackFactory;
    use zk_ee::oracle::query_ids::INITIAL_STORAGE_SLOT_VALUE_QUERY_ID;
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
    use zk_ee::storage_types::InitialStorageSlotData;
    use zk_ee::system::{Computational, Resource};
    use zk_ee::utils::Bytes32;

    type TestResources = BaseResources<DecreasingNative>;
    type TestCache = GenericPubdataAwarePlainStorage<
        WarmStorageKey,
        Bytes32,
        Global,
        VecStackFactory,
        4,
        TestResources,
        EthereumLikeStorageAccessCostModel,
    >;

    /// Answers slot queries from a fixed map (missing slots are new) and counts them
    struct CountingOracle {
        existing: BTreeMap<Bytes32, Bytes32>,
        queries: usize,
    }

    impl IOOracle for CountingOracle {
        type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            query_type: u32,
            input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            assert_eq!(query_type, INITIAL_STORAGE_SLOT_VALUE_QUERY_ID);
            self.queries += 1;
            let address = StorageAddress::<EthereumIOTypesConfig>::from_iter(&mut input.iter())
                .expect("slot query input");
            let response = match self.existing.get(&address.key) {
                Some(value) => InitialStorageSlotData::<EthereumIOTypesConfig> {
                    is_new_storage_slot: false,
                    initial_value: *value,
                },
                None => InitialStorageSlotData::<EthereumIOTypesConfig> {
                    is_new_storage_slot: true,
                    initial_value: Bytes32::ZERO,
                },
            };
            let values: Vec<_> = response.iter().collect();
            Ok(Box::new(values.into_iter()))
        }
    }

    const ADDRESS: B160 = B160::from_limbs([0x1234, 0, 0]);
    fn slot(byte: u8) -> WarmStorageKey {
        WarmStorageKey {
            address: ADDRESS,
            key: Bytes32::from_byte_fill(byte),
        }
    }
    fn new_slot() -> WarmStorageKey {
        slot(0x01)
    }
    fn existing_slot() -> WarmStorageKey {
        slot(0x02)
    }
    fn existing_value() -> Bytes32 {
        Bytes32::from_byte_fill(0xaa)
    }
    struct Address(B160);
    impl AsRef<B160> for Address {
        fn as_ref(&self) -> &B160 {
            &self.0
        }
    }

    fn setup() -> (TestCache, CountingOracle) {
        let mut cache = TestCache::new_from_parts(Global, EthereumLikeStorageAccessCostModel);
        cache.begin_new_tx();
        let oracle = CountingOracle {
            existing: BTreeMap::from([(existing_slot().key, existing_value())]),
            queries: 0,
        };
        (cache, oracle)
    }

    fn touch(cache: &mut TestCache, key: &WarmStorageKey) -> u64 {
        let mut resources = TestResources::FORMAL_INFINITE;
        resources.with_infinite_ergs(|resources| {
            cache
                .apply_touch_impl(ExecutionEnvironmentType::NoEE, key, resources)
                .expect("touch")
        });
        TestResources::FORMAL_INFINITE.native().as_u64() - resources.native().as_u64()
    }

    /// Reads as the EVM does; returns (value, gas charged, native charged)
    fn read(
        cache: &mut TestCache,
        oracle: &mut CountingOracle,
        key: &WarmStorageKey,
    ) -> (Bytes32, u64, u64) {
        let mut resources = TestResources::FORMAL_INFINITE;
        let value = cache
            .apply_read_impl(ExecutionEnvironmentType::EVM, key, &mut resources, oracle)
            .expect("read");
        let spent = TestResources::FORMAL_INFINITE.diff(resources);
        let gas = spent.ergs().0 / TestResources::from_legacy_gas_saturating(1).ergs().0;
        (value, gas, spent.native().as_u64())
    }

    fn state(cache: &TestCache, key: &WarmStorageKey) -> (ElementValues<Bytes32>, Warmth) {
        let item = cache.cache.get(key).expect("element is cached");
        (element_values(&item), item.current().warmth)
    }

    #[test]
    fn touch_declares_without_reading_and_the_first_read_pays_native_only() {
        let (mut cache, mut oracle) = setup();

        let touch_native = touch(&mut cache, &new_slot());
        assert_eq!(oracle.queries, 0, "a touch must not read the slot");
        assert_eq!(touch_native, WARM_STORAGE_READ_NATIVE_COST);
        let (values, warmth) = state(&cache, &new_slot());
        assert!(!values.is_observed);
        assert!(warmth.is_warm(cache.current_tx_id));

        let (value, gas, native) = read(&mut cache, &mut oracle, &new_slot());
        assert_eq!(oracle.queries, 1);
        assert_eq!(value, Bytes32::ZERO);
        assert_eq!(gas, WARM_STORAGE_READ_COST, "warm by EIP-2929");
        assert_eq!(
            native,
            WARM_STORAGE_READ_NATIVE_COST + COLD_NEW_STORAGE_READ_NATIVE_COST,
            "the merkle work is paid by the read that needs it"
        );
        let (values, _) = state(&cache, &new_slot());
        assert!(values.is_observed && values.is_new);

        let (_, gas, native) = read(&mut cache, &mut oracle, &new_slot());
        assert_eq!(oracle.queries, 1);
        assert_eq!(
            (gas, native),
            (WARM_STORAGE_READ_COST, WARM_STORAGE_READ_NATIVE_COST)
        );
    }

    #[test]
    fn observation_survives_the_rollback_of_the_reading_frame() {
        let (mut cache, mut oracle) = setup();
        touch(&mut cache, &existing_slot());

        let frame = cache.start_frame();
        let (value, _, _) = read(&mut cache, &mut oracle, &existing_slot());
        assert_eq!(value, existing_value());
        cache.finish_frame_impl(Some(&frame)).expect("rollback");

        let (values, warmth) = state(&cache, &existing_slot());
        assert!(values.is_observed, "a rollback can not forget a value");
        assert_eq!(values.initial, existing_value());
        assert!(
            warmth.is_warm(cache.current_tx_id),
            "the touch was outside the frame"
        );

        let (value, gas, native) = read(&mut cache, &mut oracle, &existing_slot());
        assert_eq!(oracle.queries, 1, "the value is already known");
        assert_eq!(value, existing_value());
        assert_eq!(
            (gas, native),
            (WARM_STORAGE_READ_COST, WARM_STORAGE_READ_NATIVE_COST)
        );
    }

    #[test]
    fn rolled_back_touch_and_read_leave_the_slot_cold_but_observed() {
        let (mut cache, mut oracle) = setup();

        let frame = cache.start_frame();
        touch(&mut cache, &new_slot());
        read(&mut cache, &mut oracle, &new_slot());
        cache.finish_frame_impl(Some(&frame)).expect("rollback");

        let (values, warmth) = state(&cache, &new_slot());
        assert!(values.is_observed);
        assert!(!warmth.is_warm(cache.current_tx_id));

        // cold again, and the NEW read extra was rolled back with its payer
        let (_, gas, native) = read(&mut cache, &mut oracle, &new_slot());
        assert_eq!(oracle.queries, 1);
        assert_eq!(gas, COLD_SLOAD_COST);
        assert_eq!(
            native,
            WARM_STORAGE_READ_NATIVE_COST + COLD_NEW_STORAGE_READ_NATIVE_COST
        );
    }

    #[test]
    fn touched_slot_is_cold_and_unobserved_in_the_next_transaction() {
        let (mut cache, mut oracle) = setup();
        touch(&mut cache, &existing_slot());
        cache.begin_new_tx();

        let (values, warmth) = state(&cache, &existing_slot());
        assert!(!values.is_observed);
        assert!(!warmth.is_warm(cache.current_tx_id));

        // charged exactly like a slot the cache never saw
        let (value, gas, native) = read(&mut cache, &mut oracle, &existing_slot());
        assert_eq!(oracle.queries, 1);
        assert_eq!(value, existing_value());
        assert_eq!(gas, COLD_SLOAD_COST);
        assert_eq!(
            native,
            WARM_STORAGE_READ_NATIVE_COST + COLD_EXISTING_STORAGE_READ_NATIVE_COST
        );
    }

    #[test]
    fn observed_slot_touched_in_the_next_transaction_is_warm_without_io() {
        let (mut cache, mut oracle) = setup();
        read(&mut cache, &mut oracle, &existing_slot());
        cache.begin_new_tx();

        touch(&mut cache, &existing_slot());
        assert_eq!(oracle.queries, 1);
        let (value, gas, native) = read(&mut cache, &mut oracle, &existing_slot());
        assert_eq!(oracle.queries, 1);
        assert_eq!(value, existing_value());
        assert_eq!(
            (gas, native),
            (WARM_STORAGE_READ_COST, WARM_STORAGE_READ_NATIVE_COST)
        );
    }

    #[test]
    fn write_after_touch_reads_first_and_is_warm() {
        let (mut cache, mut oracle) = setup();
        touch(&mut cache, &new_slot());

        let new_value = Bytes32::from_array([0x77; 32]);
        let mut resources = TestResources::FORMAL_INFINITE;
        let old_value = cache
            .apply_write_impl(
                ExecutionEnvironmentType::EVM,
                &new_slot(),
                &new_value,
                &mut oracle,
                &mut resources,
            )
            .expect("write");
        assert_eq!(oracle.queries, 1);
        assert_eq!(old_value, Bytes32::ZERO);
        let spent = TestResources::FORMAL_INFINITE.diff(resources);
        let gas = spent.ergs().0 / TestResources::from_legacy_gas_saturating(1).ergs().0;
        assert_eq!(
            gas,
            WARM_STORAGE_READ_COST + SSTORE_SET_EXTRA,
            "no cold surcharge for a slot warmed by the access list"
        );
        assert!(
            spent.native().as_u64()
                > WARM_STORAGE_READ_NATIVE_COST
                    + COLD_NEW_STORAGE_READ_NATIVE_COST
                    + WARM_STORAGE_WRITE_EXTRA_NATIVE_COST,
            "the read and the write merkle work are both paid"
        );

        let (values, _) = state(&cache, &new_slot());
        assert!(values.is_observed && values.is_new);
        assert_eq!((values.initial, values.current), (Bytes32::ZERO, new_value));
    }

    #[test]
    fn clearing_an_address_keeps_touched_slots_unobserved() {
        let (mut cache, mut oracle) = setup();
        touch(&mut cache, &new_slot());
        read(&mut cache, &mut oracle, &existing_slot());

        cache.clear_state_impl(Address(ADDRESS)).expect("clear");

        let (touched, _) = state(&cache, &new_slot());
        assert!(!touched.is_observed);
        let (cleared, _) = state(&cache, &existing_slot());
        assert!(cleared.is_observed);
        assert_eq!(
            (cleared.initial, cleared.current),
            (existing_value(), Bytes32::ZERO)
        );
    }
}
