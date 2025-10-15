//! Storage cache, backed by a history map.
use alloc::collections::BTreeMap;
use alloc::fmt::Debug;
use core::alloc::Allocator;
use ruint::aliases::B160;
use zk_ee::common_structs::cache_record::CacheRecord;
#[cfg(feature = "evm_refunds")]
use zk_ee::common_structs::history_counter::HistoryCounter;
#[cfg(feature = "evm_refunds")]
use zk_ee::common_structs::history_counter::HistoryCounterSnapshotId;
use zk_ee::common_structs::history_map::CacheSnapshotId;
use zk_ee::common_structs::history_map::HistoryMap;
use zk_ee::common_structs::history_map::HistoryMapItemRefMut;
use zk_ee::common_structs::structured_storage_cache_record::{
    StorageCacheAppearance, StorageCurrentAppearance, StorageInitialAppearance,
};
use zk_ee::common_traits::key_like_with_bounds::{KeyLikeWithBounds, TyEq};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::oracle::basic_queries::InitialStorageSlotQuery;
use zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::{
    memory::stack_trait::StackFactory,
    oracle::IOOracle,
    storage_types::StorageAddress,
    system::{errors::system::SystemError, Resources},
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct TransactionId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsWarmRead(pub bool);

pub(crate) type StorageItem<'a, K, V, A> =
    HistoryMapItemRefMut<'a, K, CacheRecord<V, StorageElementMetadata>, A, StorageCacheAppearance>;

/// EE-specific IO charging.
pub trait StorageAccessPolicy<R: Resources, V>: 'static + Sized {
    /// Charge for a warm read (already in cache).
    fn charge_warm_storage_read(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        is_access_list: bool,
    ) -> Result<(), SystemError>;

    /// Charge the extra cost of reading a key
    /// not present in the cache. This cost is added
    /// to the cost of a warm read.
    fn charge_cold_storage_read_extra(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        is_new_slot: bool,
    ) -> Result<(), SystemError>;

    /// Charge the additional cost of performing a write.
    /// This cost is added to the cost of reading.
    /// We assume writing is always at least as expensive
    /// as reading.
    fn charge_storage_write_extra(
        &self,
        ee_type: ExecutionEnvironmentType,
        initial_value: &V,
        current_value: &V,
        new_value: &V,
        resources: &mut R,
        is_warm_write: bool,
        is_new_slot: bool,
    ) -> Result<(), SystemError>;

    /// Refund some resources if needed
    fn refund_for_storage_write(
        &self,
        ee_type: ExecutionEnvironmentType,
        value_at_tx_start: &V,
        current_value: &V,
        new_value: &V,
        resources: &mut R,
        refund_counter: &mut R,
    ) -> Result<(), SystemError>;
}

#[derive(Default, Clone)]
pub struct StorageElementMetadata {
    /// Transaction where this account was last accessed.
    /// Considered warm if equal to Some(current_tx)
    pub last_touched_in_tx: Option<TransactionId>,
}

impl StorageElementMetadata {
    pub fn considered_warm(&self, current_tx_number: TransactionId) -> bool {
        self.last_touched_in_tx == Some(current_tx_number)
    }
}

#[derive(Debug)]
pub struct StorageSnapshotId {
    pub cache: CacheSnapshotId,
    #[cfg(feature = "evm_refunds")]
    pub evm_refunds_counter: HistoryCounterSnapshotId,
}

pub struct GenericPubdataAwareStorageValuesCache<
    K: KeyLikeWithBounds,
    V,
    A: Allocator + Clone, // = Global,
    SF: StackFactory<N>,
    const N: usize,
    R: Resources,
    P: StorageAccessPolicy<R, V>,
> {
    pub cache: HistoryMap<K, CacheRecord<V, StorageElementMetadata>, A, StorageCacheAppearance>,
    pub(crate) resources_policy: P,
    pub(crate) current_tx_number: TransactionId,
    pub(crate) initial_values: BTreeMap<K, (V, TransactionId), A>, // Used to cache initial values at the beginning of the tx (For EVM gas model)
    #[cfg(feature = "evm_refunds")]
    pub(crate) evm_refunds_counter: HistoryCounter<R, SF, N, A>, // Used to keep track of EVM gas refunds
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
        SF: StackFactory<N>,
        const N: usize,
        R: Resources,
        P: StorageAccessPolicy<R, V>,
    > GenericPubdataAwareStorageValuesCache<K, V, A, SF, N, R, P>
{
    pub fn new_from_parts(allocator: A, resources_policy: P) -> Self {
        Self {
            cache: HistoryMap::new(allocator.clone()),
            current_tx_number: TransactionId(0),
            resources_policy,
            initial_values: BTreeMap::new_in(allocator.clone()),
            #[cfg(feature = "evm_refunds")]
            evm_refunds_counter: HistoryCounter::new(allocator.clone()),
            alloc: allocator.clone(),
            _marker: core::marker::PhantomData,
        }
    }

    pub fn begin_new_tx(&mut self) {
        self.cache.commit();
        #[cfg(feature = "evm_refunds")]
        {
            self.evm_refunds_counter = HistoryCounter::new(self.alloc.clone());
            self.evm_refunds_counter.update(R::empty());
        }
    }

    pub fn finish_tx(&mut self) {
        self.current_tx_number.0 += 1;
    }

    #[track_caller]
    pub fn start_frame(&mut self) -> StorageSnapshotId {
        StorageSnapshotId {
            cache: self.cache.snapshot(),
            #[cfg(feature = "evm_refunds")]
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
            #[cfg(feature = "evm_refunds")]
            self.evm_refunds_counter.rollback(x.evm_refunds_counter);
            self.cache.rollback(x.cache)
        } else {
            Ok(())
        }
    }

    /// Read element and initialize it if needed
    pub(crate) fn materialize_element<'a>(
        cache: &'a mut HistoryMap<
            K,
            CacheRecord<V, StorageElementMetadata>,
            A,
            StorageCacheAppearance,
        >,
        resources_policy: &mut P,
        current_tx_number: TransactionId,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        key: &'a K,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(StorageItem<'a, K, V, A>, IsWarmRead), SystemError>
    where
        StorageAddress<EthereumIOTypesConfig>: From<K>,
    {
        resources_policy.charge_warm_storage_read(ee_type, resources, is_access_list)?;

        let mut initialized_element = false;

        cache
            .get_or_insert(key, || {
                // Element doesn't exist in cache yet, initialize it
                initialized_element = true;

                let query_input = (*key).into();
                let data_from_oracle = InitialStorageSlotQuery::get(oracle, &query_input)
                    .expect("must get initial slot value from oracle");

                resources_policy.charge_cold_storage_read_extra(
                    ee_type,
                    resources,
                    data_from_oracle.is_new_storage_slot,
                )?;

                let initial_appearance = match data_from_oracle.is_new_storage_slot {
                    true => StorageInitialAppearance::Empty,
                    false => StorageInitialAppearance::Existing,
                };

                let current_appearance = StorageCurrentAppearance::Observed;
                let appearance =
                    StorageCacheAppearance::new(initial_appearance, current_appearance);

                // Note: we initialize it as cold, should be warmed up separately
                // Since in case of revert it should become cold again and initial record can't be rolled back
                Ok((
                    CacheRecord::new(data_from_oracle.initial_value.into()),
                    appearance,
                ))
            })
            .and_then(|mut x| {
                // Warm up element according to EVM rules if needed
                let is_warm_read = x.current().metadata().considered_warm(current_tx_number);
                if is_warm_read == false {
                    if initialized_element == false {
                        // Element exists in cache, but wasn't touched in current tx yet
                        resources_policy
                            .charge_cold_storage_read_extra(ee_type, resources, false)?;
                    }

                    x.update(|cache_record| {
                        cache_record.update_metadata(|m| {
                            m.last_touched_in_tx = Some(current_tx_number);
                            Ok(())
                        })
                    })?;
                }

                Ok((x, IsWarmRead(is_warm_read)))
            })
    }

    pub fn apply_read_impl(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        key: &K,
        resources: &mut R,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<V, SystemError>
    where
        StorageAddress<EthereumIOTypesConfig>: From<K>,
    {
        let (addr_data, _) = Self::materialize_element(
            &mut self.cache,
            &mut self.resources_policy,
            self.current_tx_number,
            ee_type,
            resources,
            key,
            oracle,
            is_access_list,
        )?;

        Ok(addr_data.current().value().clone())
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
            self.current_tx_number,
            ee_type,
            resources,
            key,
            oracle,
            false,
        )?;

        let val_current = addr_data.current().value();

        // Try to get initial value at the beginning of the tx.
        let val_at_tx_start = match self.initial_values.entry(*key) {
            alloc::collections::btree_map::Entry::Vacant(vacant_entry) => {
                &vacant_entry
                    .insert((val_current.clone(), self.current_tx_number))
                    .0
            }
            alloc::collections::btree_map::Entry::Occupied(occupied_entry) => {
                let (value, tx_number) = occupied_entry.into_mut();
                if *tx_number != self.current_tx_number {
                    *value = val_current.clone();
                    *tx_number = self.current_tx_number;
                }
                value
            }
        };

        let is_new_slot =
            addr_data.key_properties().initial_appearance() == StorageInitialAppearance::Empty;
        self.resources_policy.charge_storage_write_extra(
            ee_type,
            val_at_tx_start,
            val_current,
            new_value,
            resources,
            is_warm_read.0,
            is_new_slot,
        )?;

        let old_value = addr_data.current().value().clone();
        addr_data.key_properties_mut().update();
        addr_data.update(|cache_record| {
            cache_record.update(|x, _| {
                *x = new_value.clone();
                Ok(())
            })
        })?;

        // we need to replace-update
        #[cfg(feature = "evm_refunds")]
        if let Some(mut refund_counter) = self.evm_refunds_counter.value().cloned() {
            self.resources_policy.refund_for_storage_write(
                ee_type,
                &val_at_tx_start,
                &old_value,
                new_value,
                resources,
                &mut refund_counter,
            )?;
            self.evm_refunds_counter.update(refund_counter);
        }

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
                x.update(|cache_record| {
                    cache_record.update(|v, _| {
                        *v = V::default();
                        Ok(())
                    })?;
                    Ok(())
                })?;
                x.key_properties_mut().delete();

                Ok(())
            })?;

        Ok(())
    }

    pub fn get_refund_counter_impl(&'_ self) -> Option<&'_ R> {
        #[cfg(feature = "evm_refunds")]
        {
            self.evm_refunds_counter.value()
        }

        #[cfg(not(feature = "evm_refunds"))]
        None
    }

    pub fn add_to_refund_counter_impl(&mut self, refund: R) -> Result<(), SystemError> {
        #[cfg(feature = "evm_refunds")]
        {
            if let Some(mut t) = self.get_refund_counter_impl().cloned() {
                t.add_ergs(refund.ergs());
                self.evm_refunds_counter.update(t);
            }

            Ok(())
        }

        #[cfg(not(feature = "evm_refunds"))]
        {
            let _ = refund;
            Ok(())
        }
    }
}
