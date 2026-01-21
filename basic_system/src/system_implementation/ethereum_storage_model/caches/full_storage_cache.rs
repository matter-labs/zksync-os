//! Storage cache, backed by a history map.
use core::alloc::Allocator;
use storage_models::common_structs::snapshottable_io::SnapshottableIo;
use storage_models::common_structs::StorageCacheModel;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::{
    common_structs::{WarmStorageKey, WarmStorageValue},
    system::{errors::system::SystemError, Resources},
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::Bytes32,
};

use crate::system_implementation::caches::generic_pubdata_aware_plain_storage::{
    GenericPubdataAwarePlainStorage, StorageSnapshotId,
};
use crate::system_implementation::caches::storage_access_policy::StorageAccessPolicy;

pub struct EthereumStorageCache<
    A: Allocator + Clone,
    SF: StackFactory<N>,
    const N: usize,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
> {
    pub(crate) slot_values:
        GenericPubdataAwarePlainStorage<WarmStorageKey, Bytes32, A, SF, N, R, P>,
}

impl<
        A: Allocator + Clone,
        SF: StackFactory<N>,
        const N: usize,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
    > StorageCacheModel for EthereumStorageCache<A, SF, N, R, P>
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
        let key = WarmStorageKey {
            address: *address,
            key: *key,
        };

        self.slot_values
            .apply_read_impl(ee_type, &key, resources, oracle)
    }

    fn touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        // TODO(EVM-1076): use a different low-level function to avoid creating pubdata
        // and merkle proof obligations until we actually read the value

        let key = WarmStorageKey {
            address: *address,
            key: *key,
        };

        self.slot_values
            .apply_read_impl(ee_type, &key, resources, oracle)?;
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
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError> {
        let key = WarmStorageKey {
            address: *address,
            key: *key,
        };

        let (old_value, _) = self
            .slot_values
            .apply_write_impl(ee_type, &key, new_value, oracle, resources)?;

        Ok(old_value)
    }

    fn read_special_account_property<T: storage_models::common_structs::SpecialAccountProperty>(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _resources: &mut Self::Resources,
        _address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        _oracle: &mut impl IOOracle,
    ) -> Result<T::Value, SystemError> {
        panic!("unreachable for such cache");
    }

    fn write_special_account_property<T: storage_models::common_structs::SpecialAccountProperty>(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _resources: &mut Self::Resources,
        _address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        _new_value: &T::Value,
        _oracle: &mut impl IOOracle,
    ) -> Result<T::Value, SystemError> {
        panic!("unreachable for such cache");
    }
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
        self.slot_values.begin_new_tx();
    }

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        self.slot_values.finish_tx();
        Ok(())
    }

    fn start_frame(&mut self) -> Self::StateSnapshot {
        self.slot_values.start_frame()
    }

    fn finish_frame(
        &mut self,
        rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError> {
        self.slot_values.finish_frame_impl(rollback_handle)
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
    pub(crate) fn iter_as_storage_types(
        &self,
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone + use<'_, A, SF, N, R, P>
    {
        self.slot_values.cache.iter().map(|item| {
            let current_record = item.current();
            let initial_record = item.initial();
            let is_new_storage_slot = item.key_properties().is_new_element();
            let initial_value_used = item.key_properties().is_value_known();
            (
                *item.key(),
                // Using the WarmStorageValue temporarily till it's outed from the codebase. We're
                // not actually 'using' it.
                WarmStorageValue {
                    current_value: *current_record.value(),
                    is_new_storage_slot,
                    initial_value: *initial_record.value(),
                    initial_value_used,
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
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone + use<'_, A, SF, N, R, P>
    {
        self.iter_as_storage_types()
    }

    ///
    /// Returns slots that were changed during execution.
    ///
    pub fn net_diffs_iter(
        &self,
    ) -> impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + use<'_, A, SF, N, R, P> {
        self.iter_as_storage_types()
            .filter(|(_, v)| v.current_value != v.initial_value)
    }

    pub fn calculate_pubdata_used_by_tx(&self) -> u32 {
        0
    }
}
