//! Storage cache, backed by a history map.
use crate::system_implementation::flat_storage_model::address_into_special_storage_key;
use crate::system_implementation::system::ExtraCheck;
use alloc::fmt::Debug;
use core::alloc::Allocator;
use ruint::aliases::B160;
use storage_models::common_structs::snapshottable_io::SnapshottableIo;
use storage_models::common_structs::{AccountAggregateDataHash, StorageCacheModel};
use zk_ee::common_traits::key_like_with_bounds::{KeyLikeWithBounds, TyEq};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::{
    common_structs::{WarmStorageKey, WarmStorageValue},
    kv_markers::{StorageAddress, UsizeDeserializable},
    memory::stack_trait::{StackCtor, StackCtorConst},
    system::{errors::SystemError, Resources},
    system_io_oracle::{IOOracle, InitialStorageSlotData, InitialStorageSlotDataIterator},
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::Bytes32,
};

use zk_ee::common_structs::history_map::*;
use zk_ee::common_structs::ValueDiffCompressionStrategy;

type AddressItem<'a, K, V, A> = CacheItemRefMut<'a, K, V, StorageElementMetadata, A>;

/// EE-specific IO charging.
pub trait StorageAccessPolicy<R: Resources, V>: 'static + Sized {
    /// Charge for a warm read (already in cache).
    fn charge_warm_storage_read(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
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
    pub last_touched_in_tx: Option<u32>,
}

impl StorageElementMetadata {
    pub fn considered_warm(&self, current_tx_number: u32) -> bool {
        self.last_touched_in_tx == Some(current_tx_number)
    }
}

pub struct GenericPubdataAwarePlainStorage<
    K: KeyLikeWithBounds,
    V,
    A: Allocator + Clone, // = Global,
    SC: StackCtor<SCC>,
    SCC: const StackCtorConst,
    R: Resources,
    P: StorageAccessPolicy<R, V>,
> where
    ExtraCheck<SCC, A>:,
{
    pub(crate) cache: HistoryMap<K, V, StorageElementMetadata, A>,
    pub(crate) resources_policy: P,
    pub(crate) current_tx_number: u32,
    pub(crate) _marker: core::marker::PhantomData<(R, SC, SCC)>,
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
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        R: Resources,
        P: StorageAccessPolicy<R, V>,
    > GenericPubdataAwarePlainStorage<K, V, A, SC, SCC, R, P>
where
    ExtraCheck<SCC, A>:,
{
    pub fn new_from_parts(allocator: A, resources_policy: P) -> Self {
        Self {
            cache: HistoryMap::new(allocator.clone()),
            current_tx_number: 0,
            resources_policy,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn begin_new_tx(&mut self) {
        self.cache.commit();

        self.current_tx_number += 1;
    }

    #[track_caller]
    pub fn start_frame(&mut self) -> CacheSnapshotId {
        self.cache
            .snapshot(TransactionId(self.current_tx_number as u64))
    }

    #[track_caller]
    pub fn finish_frame_impl(&mut self, rollback_handle: Option<&CacheSnapshotId>) {
        if let Some(x) = rollback_handle {
            self.cache.rollback(*x);
        }
    }

    fn materialize_element<'a>(
        cache: &'a mut HistoryMap<K, V, StorageElementMetadata, A>,
        resources_policy: &mut P,
        current_tx_number: u32,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &StorageAddress<EthereumIOTypesConfig>,
        key: &'a K,
        oracle: &mut impl IOOracle,
    ) -> Result<(AddressItem<'a, K, V, A>, IsWarmRead), SystemError> {
        resources_policy.charge_warm_storage_read(ee_type, resources)?;

        let mut cold_read_charged = false;

        cache
            .materialize(resources, key, |resources| {
                let mut dst =
                    core::mem::MaybeUninit::<InitialStorageSlotData<EthereumIOTypesConfig>>::uninit(
                    );
                let mut it = oracle
                    .create_oracle_access_iterator::<InitialStorageSlotDataIterator<EthereumIOTypesConfig>>(
                        *address,
                    )
                    .expect("must make an iterator");
                unsafe { UsizeDeserializable::init_from_iter(&mut dst, &mut it).expect("must initialize") };
                assert!(it.next().is_none());

                // Safety: Since the `init_from_iter` has completed successfulle and there's no
                // outstanding data as per line before, we can assume that the value was read
                // correctly.
                let data_from_oracle = unsafe { dst.assume_init() } ;

                resources_policy.charge_cold_storage_read_extra(ee_type, resources, data_from_oracle.is_new_storage_slot)?;
                cold_read_charged = true;

                let appearance = match data_from_oracle.is_new_storage_slot {
                    true => Appearance::Unset,
                    false => Appearance::Retrieved,
                };
                Ok((data_from_oracle.initial_value.into(), appearance))
            })
            // We're adding a read snapshot for case when we're rollbacking the initial read.
            .and_then(|mut x| {
                let is_warm_read = x.current().metadata.considered_warm(current_tx_number);
                if is_warm_read == false {
                    if cold_read_charged == false {
                        resources_policy.charge_cold_storage_read_extra(ee_type, resources,false)?;
                    }

                    x.update_metadata(|m| {
                        m.last_touched_in_tx = Some(current_tx_number);
                        Ok(())
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
        )?;

        Ok(addr_data.current().value.clone())
    }

    pub fn apply_write_impl(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        address: &StorageAddress<EthereumIOTypesConfig>,
        key: &K,
        new_value: &V,
        oracle: &mut impl IOOracle,
        resources: &mut R,
    ) -> Result<V, SystemError>
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
        )?;

        let (val_at_tx_start, val_current) = addr_data
            .diff_operands_tx()
            .unwrap_or((addr_data.current(), addr_data.current()));
        self.resources_policy.charge_storage_write_extra(
            ee_type,
            &val_at_tx_start.value,
            &val_current.value,
            new_value,
            resources,
            is_warm_read.0,
            addr_data.current().appearance == Appearance::Unset,
        )?;

        let old_value = addr_data.current().value.clone();
        addr_data.update(|x, _m| {
            *x = new_value.clone();
            Ok(())
        })?;

        Ok(old_value)
    }

    pub fn clear_state_impl(&mut self, address: impl AsRef<B160>) -> Result<(), SystemError>
    where
        K::Subspace: TyEq<B160>,
    {
        use core::ops::Bound::Included;
        let lower_bound = K::lower_bound(TyEq::rwi(*address.as_ref()));
        let upper_bound = K::upper_bound(TyEq::rwi(*address.as_ref()));
        self.cache
            .for_each_range((Included(&lower_bound), Included(&upper_bound)), |mut x| {
                x.unset()
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
    SC: StackCtor<SCC>,
    SCC: const StackCtorConst,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
>(pub GenericPubdataAwarePlainStorage<WarmStorageKey, Bytes32, A, SC, SCC, R, P>)
where
    ExtraCheck<SCC, A>:;

impl<
        A: Allocator + Clone,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
    > StorageCacheModel for NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>
where
    ExtraCheck<SCC, A>:,
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
            .apply_read_impl(ee_type, &sa, &key, resources, oracle)
    }

    fn write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        new_value: &<Self::IOTypes as SystemIOTypesConfig>::StorageValue,
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

        let old_value = self
            .0
            .apply_write_impl(ee_type, &sa, &key, new_value, oracle, resources)?;

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
            .apply_read_impl(ee_type, &sa, &key, resources, oracle)?;

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

        let old_value = self
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
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
    > SnapshottableIo for NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>
where
    ExtraCheck<SCC, A>:,
{
    type StateSnapshot = CacheSnapshotId;

    fn begin_new_tx(&mut self) {
        self.0.begin_new_tx();
    }

    fn start_frame(&mut self) -> Self::StateSnapshot {
        self.0.start_frame()
    }

    fn finish_frame(&mut self, rollback_handle: Option<&Self::StateSnapshot>) {
        self.0.finish_frame_impl(rollback_handle);
    }
}

impl<
        A: Allocator + Clone,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
    > NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>
where
    ExtraCheck<SCC, A>:,
{
    ///
    /// Returns all the accessed storage slots.
    ///
    /// This one should be used for merkle proof validation, includes initial reads.
    ///
    pub fn net_accesses_iter(
        &self,
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone + use<'_, A, SC, SCC, R, P>
    {
        self.0.cache.iter_as_storage_types()
    }

    ///
    /// Returns slots that were changed during execution.
    ///
    pub fn net_diffs_iter(
        &self,
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone + use<'_, A, SC, SCC, R, P>
    {
        self.0
            .cache
            .iter_as_storage_types()
            .filter(|(_, v)| v.current_value != v.initial_value)
    }

    pub fn net_pubdata_used(&self) -> u32 {
        // TODO: should be constant complexity
        let mut pubdata_used = 0u32;
        self.0
            .cache
            .for_total_diff_operands::<_, ()>(|l, r, k| {
                // TODO: use tree index instead of key for repeated writes
                pubdata_used += 32; // key
                                    // we publish preimages for account details, so no need to publish hash
                if k.address == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
                    return Ok(());
                }
                if l.value != r.value {
                    pubdata_used += ValueDiffCompressionStrategy::optimal_compression_length(
                        &l.value, &r.value,
                    ) as u32;
                }
                Ok(())
            })
            .expect("We're returning Ok(())");
        pubdata_used
    }
}
