//! Storage cache, backed by a history map, whose keys are an address and a slot key:
//! the two parts are taken separately, so callers never assemble the composite key,
//! and the map builds it once, at its lookup.
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
use zk_ee::common_structs::warm_storage_key::ComposedStorageKey;

pub use crate::system_implementation::caches::cache_element_state::TransactionId;

pub use crate::system_implementation::caches::generic_pubdata_aware_plain_storage::{
    ElementValues, IsWarmRead, StorageElementRecord, StorageSnapshotId,
};

type AddressItem<'a, K, V, A> = HistoryMapItemRefMut<'a, K, StorageElementRecord<V>, A>;

pub struct AddressedPlainStorage<
    K: KeyLikeWithBounds + ComposedStorageKey,
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
        K: 'static + KeyLikeWithBounds + ComposedStorageKey,
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
    > AddressedPlainStorage<K, V, A, SF, M, R, P>
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

    /// Reads the block-start value of the slot from the oracle
    fn read_initial_value(
        oracle: &mut impl IOOracle,
        address: &K::Address,
        key: &K::Key,
    ) -> Result<ObservedValue<V>, SystemError>
    where
        K::Address: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::Address>,
        K::Key: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::StorageKey>,
    {
        // the oracle speaks big-endian; the map may hold little-endian keys and values
        const LE: bool = crate::system_implementation::ethereum_storage_model::STORAGE_SLOTS_LE;
        let mut query_key: <EthereumIOTypesConfig as SystemIOTypesConfig>::StorageKey =
            (*key).into();
        if LE {
            query_key.bytereverse();
        }
        let query_input = StorageAddress::<EthereumIOTypesConfig> {
            address: (*address).into(),
            key: query_key,
        };
        let data_from_oracle = InitialStorageSlotQuery::get(oracle, &query_input)
            .map_err(|_| internal_error!("Must get initial slot value from oracle"))?;
        let mut initial_value = data_from_oracle.initial_value;
        if LE {
            initial_value.bytereverse();
        }
        let value: V = initial_value.into();

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
        address: &K::Address,
        key: &K::Key,
        resources: &mut R,
    ) -> Result<(), SystemError> {
        self.resources_policy
            .charge_warm_storage_read(ee_type, resources)?;

        let current_tx_id = self.current_tx_id;
        let mut item = self.cache.get_or_insert_checked_owned::<_, SystemError>(
            K::compose(address, key),
            &mut (),
            |_, _| Ok(()),
            |_| Ok((StorageElementRecord::new(ObservedValue::Unobserved), ())),
        )?;
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
        address: &'a K::Address,
        key: &'a K::Key,
        oracle: &mut impl IOOracle,
    ) -> Result<(AddressItem<'a, K, V, A>, IsWarmRead), SystemError>
    where
        K::Address: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::Address>,
        K::Key: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::StorageKey>,
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

        // the composite key exists only here, for the map: assembled once and moved in
        // if the element is new
        let mut item = cache.get_or_insert_checked_owned(
            K::compose(address, key),
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
                let value = Self::read_initial_value(*oracle, address, key)?;

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
            let value = Self::read_initial_value(oracle, address, key)?;
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
        address: &K::Address,
        key: &K::Key,
        resources: &mut R,
        oracle: &mut impl IOOracle,
    ) -> Result<V, SystemError>
    where
        K::Address: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::Address>,
        K::Key: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::StorageKey>,
    {
        let mut out = None;
        self.apply_read_impl_with(ee_type, address, key, resources, oracle, |value| {
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
        address: &K::Address,
        key: &K::Key,
        resources: &mut R,
        oracle: &mut impl IOOracle,
        place: impl FnOnce(&V),
    ) -> Result<(), SystemError>
    where
        K::Address: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::Address>,
        K::Key: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::StorageKey>,
    {
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
        address: &K::Address,
        key: &K::Key,
        new_value: &V,
        oracle: &mut impl IOOracle,
        resources: &mut R,
    ) -> Result<V, SystemError>
    where
        K::Address: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::Address>,
        K::Key: Into<<EthereumIOTypesConfig as SystemIOTypesConfig>::StorageKey>,
    {
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
