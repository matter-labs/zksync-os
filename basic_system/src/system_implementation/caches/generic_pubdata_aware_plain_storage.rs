//! Storage cache, backed by a history map.
use crate::system_implementation::caches::cache_element_properties::CacheElementProperties;
use crate::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use alloc::fmt::Debug;
use core::alloc::Allocator;
use ruint::aliases::B160;
use zk_ee::common_structs::cache_record::CacheRecord;
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

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct TransactionId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsWarmRead(pub bool);

type AddressItem<'a, K, V, A> =
    HistoryMapItemRefMut<'a, K, CacheRecord<V, StorageElementMetadata>, A, CacheElementProperties>;

#[derive(Default, Clone)]
pub struct StorageElementMetadata {
    /// Transaction where this account was last accessed.
    /// Considered warm if equal to Some(current_tx)
    pub last_touched_in_tx: Option<TransactionId>,
}

impl StorageElementMetadata {
    pub fn considered_warm(&self, current_tx_id: TransactionId) -> bool {
        self.last_touched_in_tx == Some(current_tx_id)
    }
}

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
    pub(crate) cache:
        HistoryMap<K, CacheRecord<V, StorageElementMetadata>, A, CacheElementProperties>,
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
    }

    pub fn finish_tx(&mut self) {
        self.current_tx_id.0 += 1;
    }

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

    /// Read element and initialize it if needed
    fn materialize_element<'a>(
        cache: &'a mut HistoryMap<
            K,
            CacheRecord<V, StorageElementMetadata>,
            A,
            CacheElementProperties,
        >,
        resources_policy: &mut P,
        current_tx_id: TransactionId,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &StorageAddress<EthereumIOTypesConfig>,
        key: &'a K,
        oracle: &mut impl IOOracle,
    ) -> Result<(AddressItem<'a, K, V, A>, IsWarmRead), SystemError> {
        resources_policy.charge_warm_storage_read(ee_type, resources)?;

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

                // We need to check that the initial value is default
                if data_from_oracle.is_new_storage_slot {
                    assert_eq!(
                        V::default(),
                        data_from_oracle.initial_value.into(),
                        "Initial value of empty slot must be trivial"
                    );
                }

                // Note: we initialize it as cold, should be warmed up separately
                // Since in case of revert it should become cold again and initial record can't be rolled back
                Ok((
                    CacheRecord::new(data_from_oracle.initial_value.into()),
                    CacheElementProperties::new(data_from_oracle.is_new_storage_slot, true),
                ))
            })
            .and_then(|mut x| {
                // Warm up element according to EVM rules if needed
                let is_warm_read = x.current().metadata().considered_warm(current_tx_id);
                if is_warm_read == false {
                    if initialized_element == false {
                        let is_new_storage_slot = x.element_properties().is_new_element();
                        // Element exists in cache, but wasn't touched in current tx yet
                        resources_policy.charge_cold_storage_read_extra(
                            ee_type,
                            resources,
                            is_new_storage_slot,
                        )?;
                    }

                    x.update(|cache_record| {
                        cache_record.update_metadata(|m| {
                            m.last_touched_in_tx = Some(current_tx_id);
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
    ) -> Result<V, SystemError>
where {
        let (addr_data, _) = Self::materialize_element(
            &mut self.cache,
            &mut self.resources_policy,
            self.current_tx_id,
            ee_type,
            resources,
            address,
            key,
            oracle,
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
    ) -> Result<(V, V), SystemError>
where {
        let (mut addr_data, is_warm_read) = Self::materialize_element(
            &mut self.cache,
            &mut self.resources_policy,
            self.current_tx_id,
            ee_type,
            resources,
            address,
            key,
            oracle,
        )?;

        let val_current = addr_data.current().value();

        // Try to get initial value at the beginning of the tx.
        let val_at_tx_start = addr_data.committed().value().clone();

        let is_new_slot = addr_data.element_properties().is_new_element();
        self.resources_policy.charge_storage_write_extra(
            ee_type,
            &val_at_tx_start,
            val_current,
            new_value,
            resources,
            is_warm_read.0,
            is_new_slot,
        )?;

        let old_value = addr_data.current().value().clone();
        addr_data.update(|cache_record| {
            cache_record.update(|x, _| {
                *x = new_value.clone();
                Ok(())
            })
        })?;

        // Add refund for storage
        let mut refund_counter_value = self.evm_refunds_counter.value().clone();
        self.resources_policy.refund_for_storage_write(
            ee_type,
            &val_at_tx_start,
            &old_value,
            new_value,
            resources,
            &mut refund_counter_value,
        )?;
        self.evm_refunds_counter.update(refund_counter_value);

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
                    })
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
