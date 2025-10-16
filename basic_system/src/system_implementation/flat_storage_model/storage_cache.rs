//! Storage cache, backed by a history map.
use crate::system_implementation::flat_storage_model::address_into_special_storage_key;
use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::fmt::Debug;
use core::alloc::Allocator;
use ruint::aliases::B160;
use storage_models::common_structs::snapshottable_io::SnapshottableIo;
use storage_models::common_structs::{AccountAggregateDataHash, StorageCacheModel};
use zk_ee::common_structs::cache_record::{Appearance, CacheRecord};
#[cfg(feature = "evm_refunds")]
use zk_ee::common_structs::history_counter::HistoryCounter;
#[cfg(feature = "evm_refunds")]
use zk_ee::common_structs::history_counter::HistoryCounterSnapshotId;
use zk_ee::common_traits::key_like_with_bounds::{KeyLikeWithBounds, TyEq};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::oracle::basic_queries::InitialStorageSlotQuery;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::{
    common_structs::{WarmStorageKey, WarmStorageValue},
    memory::stack_trait::StackFactory,
    oracle::simple_oracle_query::SimpleOracleQuery,
    storage_types::StorageAddress,
    system::{errors::system::SystemError, Resources},
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::Bytes32,
};

use zk_ee::common_structs::history_map::*;
use zk_ee::common_structs::ValueDiffCompressionStrategy;

type AddressItem<'a, K, V, A> =
    HistoryMapItemRefMut<'a, K, CacheRecord<V, StorageElementMetadata>, A>;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct TransactionId(pub u32);

pub struct StorageSnapshotId {
    pub cache: CacheSnapshotId,
    #[cfg(feature = "evm_refunds")]
    pub evm_refunds_counter: HistoryCounterSnapshotId,
}

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

pub struct GenericPubdataAwarePlainStorage<
    K: KeyLikeWithBounds,
    V,
    A: Allocator + Clone, // = Global,
    SF: StackFactory<M>,
    const M: usize,
    R: Resources,
    P: StorageAccessPolicy<R, V>,
> {
    pub(crate) cache: HistoryMap<K, CacheRecord<V, StorageElementMetadata>, A>,
    pub(crate) resources_policy: P,
    pub(crate) current_tx_number: TransactionId,
    pub(crate) initial_values: BTreeMap<K, (V, TransactionId), A>, // Used to cache initial values at the beginning of the tx (For EVM gas model)
    #[cfg(feature = "evm_refunds")]
    pub(crate) evm_refunds_counter: HistoryCounter<u32, SF, M, A>, // Used to keep track of EVM gas refunds
    alloc: A,
    pub(crate) _marker: core::marker::PhantomData<(R, SF)>,
}

pub struct IsWarmRead(pub bool);

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
    fn materialize_element<'a>(
        cache: &'a mut HistoryMap<K, CacheRecord<V, StorageElementMetadata>, A>,
        resources_policy: &mut P,
        current_tx_number: TransactionId,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &StorageAddress<EthereumIOTypesConfig>,
        key: &'a K,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(AddressItem<'a, K, V, A>, IsWarmRead), SystemError> {
        resources_policy.charge_warm_storage_read(ee_type, resources, is_access_list)?;

        let mut initialized_element = false;

        cache
            .get_or_insert(key, || {
                // Element doesn't exist in cache yet, initialize it
                initialized_element = true;

                let data_from_oracle = InitialStorageSlotQuery::get(oracle, &address)
                    .map_err(|_| internal_error!("Must get initial slot value from oracle"))?;

                resources_policy.charge_cold_storage_read_extra(
                    ee_type,
                    resources,
                    data_from_oracle.is_new_storage_slot,
                )?;

                let appearance = match data_from_oracle.is_new_storage_slot {
                    true => Appearance::Unset,
                    false => Appearance::Retrieved,
                };

                // Note: we initialize it as cold, should be warmed up separately
                // Since in case of revert it should become cold again and initial record can't be rolled back
                Ok(CacheRecord::new(
                    data_from_oracle.initial_value.into(),
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
        address: &StorageAddress<EthereumIOTypesConfig>,
        key: &K,
        resources: &mut R,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<V, SystemError>
where {
        let (addr_data, _) = Self::materialize_element(
            &mut self.cache,
            &mut self.resources_policy,
            self.current_tx_number,
            ee_type,
            resources,
            address,
            key,
            oracle,
            is_access_list,
        )?;

        Ok(addr_data.current().value().clone())
    }

    pub fn apply_write_impl(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        address: &StorageAddress<EthereumIOTypesConfig>,
        key: &K,
        new_value: &V,
        oracle: &mut impl IOOracle,
        resources: &mut R,
    ) -> Result<(V, &V), SystemError>
where {
        let (mut addr_data, is_warm_read) = Self::materialize_element(
            &mut self.cache,
            &mut self.resources_policy,
            self.current_tx_number,
            ee_type,
            resources,
            address,
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

        self.resources_policy.charge_storage_write_extra(
            ee_type,
            val_at_tx_start,
            val_current,
            new_value,
            resources,
            is_warm_read.0,
            addr_data.current().appearance() == Appearance::Unset,
        )?;

        let old_value = addr_data.current().value().clone();
        addr_data.update(|cache_record| {
            cache_record.update(|x, _| {
                *x = new_value.clone();
                Ok(())
            })
        })?;

        Ok((old_value, val_at_tx_start))
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
                    cache_record.unset();
                    Ok(())
                })
            })?;

        Ok(())
    }
}

/// This storage knows concrete definitions where wer store account data hashes, etc
///
/// The address of the account which storage will be used to save mapping from account addresses to
/// partial account data(nonce, code length, etc). (key is an address, value is encoded partial
/// account data).
///
pub const ACCOUNT_PROPERTIES_STORAGE_ADDRESS: B160 = B160::from_limbs([0x8003, 0, 0]);

pub struct NewStorageWithAccountPropertiesUnderHash<
    A: Allocator + Clone,
    SF: StackFactory<M>,
    const M: usize,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
>(pub GenericPubdataAwarePlainStorage<WarmStorageKey, Bytes32, A, SF, M, R, P>);

impl<
        A: Allocator + Clone,
        SF: StackFactory<M>,
        const M: usize,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
    > StorageCacheModel for NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>
{
    type IOTypes = EthereumIOTypesConfig;
    type Resources = R;

    fn read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError> {
        let sa = StorageAddress {
            address: *address,
            key: *key,
        };

        let key = WarmStorageKey {
            address: *address,
            key: *key,
        };

        self.0
            .apply_read_impl(ee_type, &sa, &key, resources, oracle, false)
    }

    fn touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError> {
        // TODO(EVM-1076): use a different low-level function to avoid creating pubdata
        // and merkle proof obligations until we actually read the value
        let sa = StorageAddress {
            address: *address,
            key: *key,
        };

        let key = WarmStorageKey {
            address: *address,
            key: *key,
        };

        self.0
            .apply_read_impl(ee_type, &sa, &key, resources, oracle, is_access_list)?;
        Ok(())
    }

    fn write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        new_value: &<Self::IOTypes as SystemIOTypesConfig>::StorageValue,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageValue, SystemError> {
        let sa = StorageAddress {
            address: *address,
            key: *key,
        };

        let key = WarmStorageKey {
            address: *address,
            key: *key,
        };

        #[allow(unused_variables)]
        let (old_value, val_at_tx_start) = self
            .0
            .apply_write_impl(ee_type, &sa, &key, new_value, oracle, resources)?;

        if ee_type == ExecutionEnvironmentType::EVM {
            // EVM specific refunds calculation
            #[cfg(feature = "evm_refunds")]
            if old_value != *new_value {
                let val_at_tx_start = *val_at_tx_start;

                let mut gas_refunds = self
                    .0
                    .evm_refunds_counter
                    .value()
                    .copied()
                    .unwrap_or_default();

                if old_value == val_at_tx_start {
                    if !val_at_tx_start.is_zero() && new_value.is_zero() {
                        gas_refunds += 4800
                    }
                } else {
                    if !val_at_tx_start.is_zero() {
                        if old_value.is_zero() {
                            gas_refunds -= 4800
                        } else if new_value.is_zero() {
                            gas_refunds += 4800
                        }
                    }
                    if *new_value == val_at_tx_start {
                        if val_at_tx_start.is_zero() {
                            gas_refunds += 20000 - 100
                        } else {
                            gas_refunds += 5000 - 2100 - 100
                        }
                    }
                }

                self.0.evm_refunds_counter.update(gas_refunds);
            }
        }

        Ok(old_value)
    }

    fn read_special_account_property<T: storage_models::common_structs::SpecialAccountProperty>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
    ) -> Result<T::Value, SystemError> {
        if core::any::TypeId::of::<T>() != core::any::TypeId::of::<AccountAggregateDataHash>() {
            panic!("unsupported property type in this model");
        }
        // this is the only tricky part, and the only special account property that we support is a hash
        // of the total account properties

        let key = address_into_special_storage_key(address);

        // we just need to create a proper access function

        let sa = StorageAddress {
            address: ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
            key,
        };

        let key = WarmStorageKey {
            address: ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
            key,
        };

        let raw_value = self
            .0
            .apply_read_impl(ee_type, &sa, &key, resources, oracle, false)?;

        let value = unsafe {
            // we checked TypeId above, so we reinterpret. No drop/forget needed
            core::ptr::read((&raw_value as *const Bytes32).cast::<T::Value>())
        };

        Ok(value)
    }

    fn write_special_account_property<T: storage_models::common_structs::SpecialAccountProperty>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        new_value: &T::Value,
        oracle: &mut impl IOOracle,
    ) -> Result<T::Value, SystemError> {
        if core::any::TypeId::of::<T>() != core::any::TypeId::of::<AccountAggregateDataHash>() {
            panic!("unsupported property type in this model");
        }
        // this is the only tricky part, and the only special account property that we support is a hash
        // of the total account properties

        let key = address_into_special_storage_key(address);

        let sa = StorageAddress {
            address: ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
            key,
        };

        let key = WarmStorageKey {
            address: ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
            key,
        };

        let new_value = unsafe {
            // we checked TypeId above, so we reinterpret. No drop/forget needed
            core::ptr::read((new_value as *const T::Value).cast::<Bytes32>())
        };

        let (old_value, _) = self
            .0
            .apply_write_impl(ee_type, &sa, &key, &new_value, oracle, resources)?;

        let old_value = unsafe {
            // we checked TypeId above, so we reinterpret. No drop/forget needed
            core::ptr::read((&old_value as *const Bytes32).cast::<T::Value>())
        };

        Ok(old_value)
    }
}

impl<
        A: Allocator + Clone,
        SF: StackFactory<M>,
        const M: usize,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
    > SnapshottableIo for NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>
{
    type StateSnapshot = StorageSnapshotId;

    fn begin_new_tx(&mut self) {
        self.0.begin_new_tx();
    }

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        self.0.finish_tx();
        Ok(())
    }

    fn start_frame(&mut self) -> Self::StateSnapshot {
        self.0.start_frame()
    }

    fn finish_frame(
        &mut self,
        rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError> {
        self.0.finish_frame_impl(rollback_handle)
    }
}

impl<
        A: Allocator + Clone,
        SF: StackFactory<M>,
        const M: usize,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
    > NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>
{
    pub fn iter_as_storage_types(
        &self,
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone + use<'_, A, SF, M, R, P>
    {
        self.0.cache.iter().map(|item| {
            let current_record = item.current();
            let initial_record = item.initial();
            (
                *item.key(),
                // Using the WarmStorageValue temporarily till it's outed from the codebase. We're
                // not actually 'using' it.
                WarmStorageValue {
                    current_value: *current_record.value(),
                    is_new_storage_slot: initial_record.appearance() == Appearance::Unset,
                    initial_value: *initial_record.value(),
                    initial_value_used: true,
                    ..Default::default()
                },
            )
        })
    }
    ///
    /// Returns all the accessed storage slots.
    ///
    /// This one should be used for merkle proof validation, includes initial reads.
    ///
    pub fn net_accesses_iter(
        &self,
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone + use<'_, A, SF, M, R, P>
    {
        self.iter_as_storage_types()
    }

    ///
    /// Returns slots that were changed during execution.
    ///
    pub fn net_diffs_iter(
        &self,
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + use<'_, A, SF, M, R, P> {
        self.iter_as_storage_types()
            .filter(|(_, v)| v.current_value != v.initial_value)
    }

    pub fn calculate_pubdata_used_by_tx(&self) -> u32 {
        let mut visited_elements = BTreeSet::new_in(self.0.alloc.clone());

        let mut pubdata_used = 0u32;
        for element_history in self.0.cache.iter_altered_since_commit() {
            // Elements are sorted chronologically

            let element_key = element_history.key();

            // we publish preimages for account details, so no need to publish hash
            if element_key.address == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
                continue;
            }

            // Skip if already calculated pubdata for this element
            if visited_elements.contains(element_key) {
                continue;
            }
            visited_elements.insert(element_key);

            let current_value = element_history.current().value();
            let initial_value = element_history.initial().value();

            if initial_value != current_value {
                // TODO(EVM-1074): use tree index instead of key for repeated writes
                pubdata_used += 32; // key
                pubdata_used += ValueDiffCompressionStrategy::optimal_compression_length(
                    initial_value,
                    current_value,
                ) as u32;
            }
        }

        pubdata_used
    }
}
