//! Account cache, backed by a history map.
//! This caches the actual account data, which will
//! then be published into the preimage storage.
use super::BytecodeAndAccountDataPreimagesStorage;
use super::NewStorageWithAccountPropertiesUnderHash;
use crate::cost_constants::blake2s_native_cost;
use crate::system_functions::keccak256::keccak256_native_cost;
use crate::system_implementation::caches::basic_account_properties::BasicAccountPropertiesMetadata;
use crate::system_implementation::caches::cache_element_properties::CacheElementProperties;
use crate::system_implementation::flat_storage_model::account_cache_entry::AccountProperties;
use crate::system_implementation::flat_storage_model::bytecode_padding_len;
use crate::system_implementation::flat_storage_model::cost_constants::*;
use crate::system_implementation::flat_storage_model::PreimageRequest;
use crate::system_implementation::flat_storage_model::StorageAccessPolicy;
use alloc::collections::BTreeSet;
use core::alloc::Allocator;
use core::marker::PhantomData;
use evm_interpreter::errors::EvmSubsystemError;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::B160;
use ruint::aliases::U256;
use storage_models::common_structs::AccountAggregateDataHash;
use storage_models::common_structs::PreimageCacheModel;
use storage_models::common_structs::StorageCacheModel;
use zk_ee::common_structs::cache_record::CacheRecord;
use zk_ee::common_structs::history_map::CacheSnapshotId;
use zk_ee::common_structs::history_map::HistoryMap;
use zk_ee::common_structs::history_map::HistoryMapItemRefMut;
use zk_ee::common_structs::PreimageType;
use zk_ee::define_subsystem;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::interface_error;
use zk_ee::internal_error;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::system::BalanceSubsystemError;
use zk_ee::system::Computational;
use zk_ee::system::DeconstructionSubsystemError;
use zk_ee::system::NonceError;
use zk_ee::system::NonceSubsystemError;
use zk_ee::system::Resource;
use zk_ee::system::EIP7702_DELEGATION_MARKER;
use zk_ee::utils::BitsOrd;
use zk_ee::utils::Bytes32;
use zk_ee::wrap_error;
use zk_ee::{
    oracle::IOOracle,
    system::{
        errors::{internal::InternalError, system::SystemError},
        AccountData, AccountDataRequest, Ergs, IOResultKeeper, Maybe, Resources,
    },
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
};

pub type BitsOrd160 = BitsOrd<{ B160::BITS }, { B160::LIMBS }>;

/// Extension of basic properties
#[derive(Default, Clone)]
pub struct AccountPropertiesMetadata {
    pub basic: BasicAccountPropertiesMetadata,
    /// Whether this account's block-start properties preimage was admitted to
    /// the transaction-local preimage budget. This is rollback-aware because
    /// the decoded account-cache entry itself survives a dropped transaction.
    pub initial_preimage_admitted: bool,
    /// Special flag that allows avoiding publishing bytecode for deployed account.
    /// In practice, it can be set to `true` only during special protocol upgrade txs.
    /// For protocol upgrades it's ensured by governance that bytecodes are already published separately.
    pub not_publish_bytecode: bool,
    /// Special flag to not compress balance diff for pubdata size estimation.
    /// It's used to have a conservative approximation of pubdata in simulation,
    /// when due to the gas price being set to 0 there might not be a diff.
    pub not_compress_balance: bool,
}

type AddressItem<'a, A> = HistoryMapItemRefMut<
    'a,
    BitsOrd<160, 3>,
    CacheRecord<AccountProperties, AccountPropertiesMetadata>,
    A,
    CacheElementProperties,
>;

pub struct NewModelAccountCache<
    A: Allocator + Clone, // = Global,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
    SF: StackFactory<M>,
    const M: usize,
> {
    pub(crate) cache: HistoryMap<
        BitsOrd160,
        CacheRecord<AccountProperties, AccountPropertiesMetadata>,
        A,
        CacheElementProperties,
    >,
    // Note: this doesn't need to be equal to the actual tx number in the block, it just needs to be able to differentiate between transactions.
    pub(crate) current_tx_id: u32,
    alloc: A,
    phantom: PhantomData<(R, P, SF)>,
}

impl<
        A: Allocator + Clone,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SF: StackFactory<M>,
        const M: usize,
    > NewModelAccountCache<A, R, P, SF, M>
{
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            cache: HistoryMap::new(allocator.clone()),
            current_tx_id: 0,
            alloc: allocator.clone(),
            phantom: PhantomData,
        }
    }

    fn charge_ergs_for_cold_access(
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        is_selfdestruct: bool,
    ) -> Result<(), SystemError> {
        match ee_type {
            ExecutionEnvironmentType::NoEE => {}
            ExecutionEnvironmentType::EVM => {
                let cost: R = if evm_interpreter::utils::is_precompile(&address) {
                    R::empty() // We've charged the access already.
                } else {
                    let mut cost = R::from_ergs(COLD_PROPERTIES_ACCESS_EXTRA_COST_ERGS);
                    if is_selfdestruct {
                        // Selfdestruct doesn't charge for warm, but it
                        // includes the warm cost for cold access
                        cost.add_ergs(WARM_PROPERTIES_ACCESS_COST_ERGS)
                    }
                    cost
                };

                resources.charge(&cost)?;
            }
        }
        Ok(())
    }

    fn charge_native_for_cold_access(
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        charge_as_new: bool,
        policy: &P,
    ) -> Result<(), SystemError> {
        // We charge for 2 things:
        // 1. Performing the special access for account properties
        // 2. Decommitting the account properties

        // 1. Charging for special access
        resources.with_infinite_ergs(|res: &mut R| {
            // Access list only matters for ergs, we set it to false
            policy.charge_warm_storage_read(ee_type, res)
        })?;
        resources.with_infinite_ergs(|res| {
            // A new (empty at block start) account is a new slot in the tree.
            policy.charge_cold_storage_read_extra(ee_type, res, charge_as_new)
        })?;

        // 2. Charging the decommitment. When charging as NEW there is no
        // properties preimage to decommit.
        if !charge_as_new {
            BytecodeAndAccountDataPreimagesStorage::<R, A>::charge_decommitment_native_cost(
                resources,
                AccountProperties::ENCODED_SIZE,
            )?;
        }

        Ok(())
    }

    fn charge_account_persist_cost_if_needed(
        current_tx_id: u32,
        account_data: &mut AddressItem<'_, A>,
        resources: &mut R,
    ) -> Result<(), SystemError> {
        let already_charged = account_data
            .current()
            .metadata()
            .basic
            .persist_charged_in_tx
            == Some(current_tx_id);
        if already_charged {
            return Ok(());
        }

        // Use NEW cost only for the first persist charge on a truly new account.
        // Once the insertion cost has been paid (by any tx in this block), subsequent
        // txs pay EXISTING — the tree insertion is a one-time cost per block.
        let is_new = account_data.element_properties().is_new_element()
            && account_data
                .current()
                .metadata()
                .basic
                .persist_charged_in_tx
                .is_none();
        let write_cost = if is_new {
            ACCOUNT_PERSIST_NEW_WRITE_NATIVE_COST
        } else {
            ACCOUNT_PERSIST_EXISTING_WRITE_NATIVE_COST
        };
        let preimage_hash_cost = blake2s_native_cost(AccountProperties::ENCODED_SIZE);
        let total = write_cost + preimage_hash_cost;

        resources.charge(&R::from_native(R::Native::from_computational(total)))?;

        account_data.update(|cache_record| {
            cache_record.update_metadata(|m| {
                m.basic.persist_charged_in_tx = Some(current_tx_id);
                Ok(())
            })
        })?;

        Ok(())
    }

    /// Read element and initialize it if needed
    fn materialize_element<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
        observe: bool,
    ) -> Result<AddressItem<'_, A>, SystemError> {
        let ergs = match ee_type {
            ExecutionEnvironmentType::NoEE => Ergs::empty(),
            ExecutionEnvironmentType::EVM =>
            // For selfdestruct, there's no warm access cost
            {
                if is_selfdestruct {
                    Ergs::empty()
                } else {
                    WARM_PROPERTIES_ACCESS_COST_ERGS
                }
            }
        };
        let native = R::Native::from_computational(WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST);
        resources.charge(&R::from_ergs_and_native(ergs, native))?;

        // Conservative pre-gate for a cold access: the property IO / decommit in
        // the insertion closure run on infinite resources and the real cold charge
        // only happens at warm-up, so without this a tx could force that (prover)
        // work and then fail the charge, leaving it unpaid. So we check up front
        // that the worst-case cold access is affordable. Charging a throwaway copy
        // of the resources here is just a way to check we have enough — nothing is
        // spent, the real charge still happens at warm-up. Charging the new-account
        // branch (the larger, extra-merkle-path one) plus a properties decommit
        // upper-bounds either branch the IO ends up taking, so the real warm-up
        // charge below cannot fail. The bound is data-independent and warmth is
        // rollback-aware, so the gate is identical across sequencer and proving and
        // never depends on cache-entry presence.
        let current_tx_id = self.current_tx_id;
        let (is_cold, is_missing) = match self.cache.get(address.into()) {
            Some(item) => (
                !item
                    .current()
                    .metadata()
                    .basic
                    .considered_warm(current_tx_id),
                false,
            ),
            None => (true, true),
        };
        if is_cold {
            let mut probe = resources.clone();
            Self::charge_ergs_for_cold_access(ee_type, &mut probe, address, is_selfdestruct)?;
            Self::charge_native_for_cold_access(
                ee_type,
                &mut probe,
                true,
                &storage.0.resources_policy,
            )?;
            BytecodeAndAccountDataPreimagesStorage::<R, A>::charge_decommitment_native_cost(
                &mut probe,
                AccountProperties::ENCODED_SIZE,
            )?;
        }

        // Every cached account can produce at most one final account snapshot.
        // Precharge it before materialization so raw entries retained by this or
        // later invalid candidates can never consume finalization headroom.
        if is_missing {
            preimages_cache
                .reserve_preimage_for_block_finalization(AccountProperties::ENCODED_SIZE)?;
        }

        self.cache
            .get_or_insert(address.into(), || {
                // Element doesn't exist in cache yet, initialize it.
                // Cold access charging happens at warm-up below: the initial
                // record persists even if the inserting transaction is dropped
                // from the block, so anything charging-related must live in
                // rollback-aware metadata.

                // We use infinite resources to perform IO. This costs are charged
                // every time we charge for "cold" access, to avoid native charging
                // depending on the state of caches.
                let mut inf_resources = R::FORMAL_INFINITE;

                // to avoid divergence we read as-if infinite ergs
                let hash = storage.read_special_account_property::<AccountAggregateDataHash>(
                    ExecutionEnvironmentType::NoEE,
                    &mut inf_resources,
                    address,
                    oracle,
                )?;

                let empty_account = hash == Bytes32::ZERO;

                let acc_data = match empty_account {
                    true => AccountProperties::default(),
                    false => {
                        let preimage = preimages_cache.get_preimage::<PROOF_ENV>(
                            ee_type,
                            &PreimageRequest {
                                hash,
                                expected_preimage_len_in_bytes: AccountProperties::ENCODED_SIZE
                                    as u32,
                                preimage_type: PreimageType::AccountData,
                            },
                            &mut inf_resources,
                            oracle,
                        )?;
                        // it's redundant as preimages cache should just check it, but why not
                        assert_eq!(preimage.len(), AccountProperties::ENCODED_SIZE);

                        AccountProperties::decode(preimage.try_into().map_err(|_| {
                            internal_error!("Unexpected preimage length for AccountProperties")
                        })?)
                    }
                };

                // Note: we initialize it as cold, should be warmed up separately
                // Since in case of revert it should become cold again and initial record can't be rolled back
                Ok((
                    CacheRecord::new(acc_data),
                    CacheElementProperties::new(empty_account, observe),
                ))
            })
            .and_then(|mut x| {
                // Warm up element according to EVM rules if needed
                let is_warm = x
                    .current()
                    .metadata()
                    .basic
                    .considered_warm(self.current_tx_id);
                if observe {
                    x.element_properties_mut().mark_value_as_observed();
                }
                if is_warm == false {
                    // The initial account-cache record survives a dropped
                    // candidate. If it represents an existing account, the
                    // decoded value can bypass `get_preimage` on the next
                    // candidate, so explicitly re-admit its backing preimage.
                    // This keeps the per-transaction cache budget identical to
                    // proving, where the dropped candidate never populated
                    // either cache.
                    let existing_account = !x.element_properties().is_new_element();
                    if !is_missing
                        && existing_account
                        && !x.current().metadata().initial_preimage_admitted
                    {
                        let initial_preimage_hash = x.initial().value().compute_hash();
                        preimages_cache.admit_cached_preimage_for_current_tx(
                            &initial_preimage_hash,
                            AccountProperties::ENCODED_SIZE,
                        )?;
                    }
                    Self::charge_ergs_for_cold_access(
                        ee_type,
                        resources,
                        address,
                        is_selfdestruct,
                    )?;
                    // The NEW read extra (tree non-inclusion check) is charged once
                    // per account per block; later cold accesses pay EXISTING plus
                    // decommitment. "Already paid" is tracked in metadata so that it
                    // rolls back together with the paying transaction if it's dropped
                    // from the block.
                    let charge_as_new = x.element_properties().is_new_element()
                        && !x.current().metadata().basic.new_read_extra_charged;
                    Self::charge_native_for_cold_access(
                        ee_type,
                        resources,
                        charge_as_new,
                        &storage.0.resources_policy,
                    )?;

                    x.update(|cache_record| {
                        cache_record.update_metadata(|m| {
                            m.basic.last_touched_in_tx = Some(self.current_tx_id);
                            if charge_as_new {
                                m.basic.new_read_extra_charged = true;
                            }
                            if existing_account {
                                m.initial_preimage_admitted = true;
                            }
                            Ok(())
                        })
                    })?;
                }
                Ok(x)
            })
    }

    fn update_nominal_token_value_inner<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        update_fn: impl FnOnce(&U256) -> Result<U256, BalanceSubsystemError>,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
        fee_payment_in_simulation: bool,
    ) -> Result<U256, BalanceSubsystemError> {
        let cur_tx = self.current_tx_id;
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            storage,
            preimages_cache,
            oracle,
            is_selfdestruct,
            true,
        )?;

        let cur = account_data.current().value().balance;
        let new = update_fn(&cur)?;

        if new != cur {
            Self::charge_account_persist_cost_if_needed(cur_tx, &mut account_data, resources)?;
        }
        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        account_data.update(|cache_record| {
            cache_record.update(|v, m| {
                v.balance = new;
                // Once an account's balance has been affected by fee
                // payment, we keep this flag set.
                m.not_compress_balance |= fee_payment_in_simulation;
                Ok(())
            })
        })?;

        Ok(cur)
    }

    fn transfer_nominal_token_value_inner<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        from: &B160,
        to: &B160,
        amount: &U256,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
    ) -> Result<(), BalanceSubsystemError> {
        use zk_ee::system::BalanceError;

        let mut f = |addr, op: fn(U256, U256) -> (U256, bool), err| {
            self.update_nominal_token_value_inner::<PROOF_ENV>(
                from_ee,
                resources,
                addr,
                move |old_balance: &U256| {
                    let (new_value, of) = op(*old_balance, *amount);
                    if of {
                        Err(err)
                    } else {
                        Ok(new_value)
                    }
                },
                storage,
                preimages_cache,
                oracle,
                is_selfdestruct,
                false, // fee_payment_in_simulation
            )
        };

        // can do update twice
        f(
            from,
            U256::overflowing_sub,
            interface_error!(BalanceError::InsufficientBalance),
        )?;
        f(
            to,
            U256::overflowing_add,
            interface_error!(BalanceError::Overflow),
        )?;

        Ok(())
    }

    // special method, not part of the trait as it's not overly generic
    pub fn persist_changes(
        &self,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) -> Result<(), SystemError> {
        self.cache.apply_to_all_updated_elements(|l, r, addr| {
            if l.value() == r.value() {
                return Ok(());
            }
            // We don't care of the left side, since we're storing the entire snapshot.
            let encoding = r.value().encoding();
            let properties_hash = r.value().compute_hash();

            // Not part of a transaction, should be included in other costs.
            let mut inf_resources = R::FORMAL_INFINITE;

            let _ = preimages_cache.record_preimage_for_block_finalization(
                &(PreimageRequest {
                    hash: properties_hash,
                    expected_preimage_len_in_bytes: AccountProperties::ENCODED_SIZE as u32,
                    preimage_type: PreimageType::AccountData,
                }),
                &mut inf_resources,
                &[&encoding],
            )?;

            storage.write_special_account_property::<AccountAggregateDataHash>(
                ExecutionEnvironmentType::NoEE,
                &mut inf_resources,
                &addr.0,
                &properties_hash,
                oracle,
            )?;

            Ok(())
        })
    }

    pub fn calculate_pubdata_used_by_tx(&self) -> u32 {
        let mut visited_elements = BTreeSet::new_in(self.alloc.clone());

        let mut pubdata_used = 0u32;
        for element_history in self.cache.iter_altered_since_commit() {
            // Elements are sorted chronologically

            let element_key = element_history.key();

            // Skip if already calculated pubdata for this element
            if visited_elements.contains(element_key) {
                continue;
            }
            visited_elements.insert(element_key);

            let current = element_history.current();
            let initial = element_history.initial();
            let at_tx_start = element_history.committed();

            // If the current value is resetting to the initial one,
            // we don't consider this diff in the pubdata charging.
            // This change will be optimized away, so it's actually reducing
            // pubdata.
            if current.value() == initial.value() && !current.metadata().not_compress_balance {
                continue;
            }

            if current.value() != at_tx_start.value() || current.metadata().not_compress_balance {
                pubdata_used += 32; // key
                pubdata_used += AccountProperties::diff_compression_length(
                    at_tx_start.value(),
                    current.value(),
                    current.metadata().not_publish_bytecode,
                    current.metadata().not_compress_balance,
                )
                .unwrap();
            }
        }

        pubdata_used
    }

    pub fn begin_new_tx(&mut self) {
        self.cache.commit();
        // Advance the warmth id at the start of each tx (not at finish) so that
        // block-level system operations, which run before the first `begin_new_tx`,
        // keep tx id 0 and are never considered warm by user transactions
        // (which start at id 1).
        self.current_tx_id += 1;
    }

    pub fn start_frame(&mut self) -> CacheSnapshotId {
        self.cache.snapshot()
    }

    #[must_use]
    pub fn finish_frame(
        &mut self,
        rollback_handle: Option<&CacheSnapshotId>,
    ) -> Result<(), InternalError> {
        if let Some(x) = rollback_handle {
            self.cache.rollback(*x)
        } else {
            Ok(())
        }
    }

    pub fn read_account_balance_assuming_warm(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &<EthereumIOTypesConfig as SystemIOTypesConfig>::Address,
    ) -> Result<<EthereumIOTypesConfig as SystemIOTypesConfig>::NominalTokenValue, SystemError>
    {
        // Charge for gas
        match ee_type {
            ExecutionEnvironmentType::NoEE => (),
            ExecutionEnvironmentType::EVM => {
                resources.charge(&R::from_ergs(KNOWN_TO_BE_WARM_PROPERTIES_ACCESS_COST_ERGS))?
            }
        }

        match self.cache.get(address.into()) {
            Some(cache_item) => Ok(cache_item.current().value().balance),
            None => Err(internal_error!("Balance assumed warm but not in cache").into()),
        }
    }

    pub fn touch_account<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        self.materialize_element::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            storage,
            preimages_cache,
            oracle,
            false,
            false,
        )?;
        Ok(())
    }

    pub fn read_account_properties<
        const PROOF_ENV: bool,
        EEVersion: Maybe<u8>,
        ObservableBytecodeHash: Maybe<<EthereumIOTypesConfig as SystemIOTypesConfig>::BytecodeHashValue>,
        ObservableBytecodeLen: Maybe<u32>,
        Nonce: Maybe<u64>,
        BytecodeHash: Maybe<<EthereumIOTypesConfig as SystemIOTypesConfig>::BytecodeHashValue>,
        BytecodeLen: Maybe<u32>,
        ArtifactsLen: Maybe<u32>,
        NominalTokenBalance: Maybe<<EthereumIOTypesConfig as SystemIOTypesConfig>::NominalTokenValue>,
        Bytecode: Maybe<&'static [u8]>,
        CodeVersion: Maybe<u8>,
        IsDelegated: Maybe<bool>,
    >(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        _request: AccountDataRequest<
            AccountData<
                EEVersion,
                ObservableBytecodeHash,
                ObservableBytecodeLen,
                Nonce,
                BytecodeHash,
                BytecodeLen,
                ArtifactsLen,
                NominalTokenBalance,
                Bytecode,
                CodeVersion,
                IsDelegated,
            >,
        >,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<
        AccountData<
            EEVersion,
            ObservableBytecodeHash,
            ObservableBytecodeLen,
            Nonce,
            BytecodeHash,
            BytecodeLen,
            ArtifactsLen,
            NominalTokenBalance,
            Bytecode,
            CodeVersion,
            IsDelegated,
        >,
        SystemError,
    > {
        let account_data = self.materialize_element::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            storage,
            preimages_cache,
            oracle,
            false,
            true,
        )?;

        let full_data = account_data.current().value();

        // we already charged for "cold" case, and now can charge more precisely

        // NOTE: we didn't yet decommit the bytecode, BUT charged for it (all properties are warm at
        // once or not), so if we do not access it ever we will not need to pollute preimages cache

        Ok(AccountData {
            ee_version: Maybe::construct(|| full_data.versioning_data.ee_version()),
            observable_bytecode_hash: Maybe::construct(|| full_data.observable_bytecode_hash),
            observable_bytecode_len: Maybe::construct(|| full_data.observable_bytecode_len),
            nonce: Maybe::construct(|| full_data.nonce),
            bytecode_hash: Maybe::construct(|| full_data.bytecode_hash),
            unpadded_code_len: Maybe::construct(|| full_data.unpadded_code_len),
            artifacts_len: Maybe::construct(|| full_data.artifacts_len),
            nominal_token_balance: Maybe::construct(|| full_data.balance),
            bytecode: Maybe::try_construct(|| {
                // we charged for "cold" behavior already, so we just ask for preimage

                if full_data.bytecode_hash.is_zero() {
                    assert!(full_data.observable_bytecode_hash.is_zero());
                    assert_eq!(full_data.unpadded_code_len, 0);
                    assert_eq!(full_data.artifacts_len, 0);
                    assert_eq!(full_data.observable_bytecode_len, 0);

                    let res: &'static [u8] = &[];
                    Ok(res)
                } else {
                    // can try to get preimage
                    let preimage_type = PreimageRequest {
                        hash: full_data.bytecode_hash,
                        expected_preimage_len_in_bytes: full_data.full_bytecode_len(),
                        preimage_type: PreimageType::Bytecode,
                    };
                    preimages_cache.get_preimage::<PROOF_ENV>(
                        ee_type,
                        &preimage_type,
                        resources,
                        oracle,
                    )
                }
            })?,
            code_version: Maybe::construct(|| full_data.versioning_data.code_version()),
            is_delegated: Maybe::try_construct(|| {
                let delegated = full_data.versioning_data.is_delegated();
                // Delegated accounts can only be of EVM EE type.
                // Note that delegates can be of any EE type, the restriction
                // is just on the delegated account itself.
                if delegated
                    && full_data.versioning_data.ee_version()
                        != ExecutionEnvironmentType::EVM_EE_BYTE
                {
                    Err(SystemError::from(internal_error!(
                        "Delegated account is not EVM"
                    )))
                } else {
                    Ok(delegated)
                }
            })?,
        })
    }

    pub fn increment_nonce<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        increment_by: u64,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<u64, NonceSubsystemError> {
        let cur_tx = self.current_tx_id;
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            storage,
            preimages_cache,
            oracle,
            false,
            true,
        )?;

        Self::charge_account_persist_cost_if_needed(cur_tx, &mut account_data, resources)?;

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        let nonce = account_data.current().value().nonce;
        if let Some(new_nonce) = nonce.checked_add(increment_by) {
            account_data.update(|cache_record| {
                cache_record.update(|x, _| {
                    x.nonce = new_nonce;
                    Ok(())
                })
            })?;
        } else {
            return Err(interface_error!(NonceError::NonceOverflow));
        }

        Ok(nonce)
    }

    pub fn update_nominal_token_value<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        update_fn: impl FnOnce(&U256) -> Result<U256, BalanceSubsystemError>,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
        fee_payment_in_simulation: bool,
    ) -> Result<U256, BalanceSubsystemError> {
        self.update_nominal_token_value_inner::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            update_fn,
            storage,
            preimages_cache,
            oracle,
            false,
            fee_payment_in_simulation,
        )
    }

    pub fn transfer_nominal_token_value<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        from: &B160,
        to: &B160,
        amount: &U256,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(), BalanceSubsystemError> {
        self.transfer_nominal_token_value_inner::<PROOF_ENV>(
            from_ee,
            resources,
            from,
            to,
            amount,
            storage,
            preimages_cache,
            oracle,
            false,
        )
    }

    fn compute_bytecode_hash(
        from_ee: ExecutionEnvironmentType,
        observable_bytecode: &[u8],
        artifacts: &[u8],
        resources: &mut R,
    ) -> Result<Bytes32, SystemError> {
        match from_ee {
            ExecutionEnvironmentType::NoEE => {
                Err(internal_error!("Deployment cannot happen in NoEE").into())
            }
            ExecutionEnvironmentType::EVM => {
                use crypto::blake2s::Blake2s256;
                use crypto::MiniDigest;
                let preimage_len = observable_bytecode.len()
                    + bytecode_padding_len(observable_bytecode.len())
                    + artifacts.len();
                let native_cost = blake2s_native_cost(preimage_len);
                resources.charge(&R::from_native(R::Native::from_computational(native_cost)))?;
                let mut hasher = Blake2s256::new();
                let padding = [0u8; core::mem::size_of::<u64>() - 1];
                hasher.update(observable_bytecode);
                hasher.update(&padding[..bytecode_padding_len(observable_bytecode.len())]);
                hasher.update(artifacts);
                Ok(Bytes32::from_array(hasher.finalize()))
            }
        }
    }

    /// Note: it is the caller's responsibility to check that the address is can be used for deployment (e.g. it is empty)
    pub fn deploy_code<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: &B160,
        deployed_code: &[u8],
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(&'static [u8], Bytes32, u32), SystemError> {
        let alloc = self.alloc.clone();
        // Charge for code deposit cost
        match from_ee {
            ExecutionEnvironmentType::NoEE => (),
            ExecutionEnvironmentType::EVM => {
                use evm_interpreter::gas_constants::CODEDEPOSIT;
                let code_deposit_cost = CODEDEPOSIT.saturating_mul(deployed_code.len() as u64);
                let ergs_to_spend = Ergs(code_deposit_cost.saturating_mul(ERGS_PER_GAS));
                resources.charge(&R::from_ergs(ergs_to_spend))?;
            }
        }

        // we charged for everything, and so all IO below will use infinite ergs

        let cur_tx = self.current_tx_id;

        let mut account_data = resources.with_infinite_ergs(|inf_resources| {
            self.materialize_element::<PROOF_ENV>(
                from_ee,
                inf_resources,
                at_address,
                storage,
                preimages_cache,
                oracle,
                false,
                true,
            )
        })?;

        Self::charge_account_persist_cost_if_needed(cur_tx, &mut account_data, resources)?;

        // compute observable and true hashes of bytecode
        let observable_bytecode_hash = match from_ee {
            ExecutionEnvironmentType::NoEE => {
                return Err(internal_error!("Deployment cannot happen in NoEE").into())
            }
            ExecutionEnvironmentType::EVM => {
                let native_cost = keccak256_native_cost::<R>(deployed_code.len());
                resources.charge(&R::from_native(native_cost))?;
                use crypto::sha3::Keccak256;
                use crypto::MiniDigest;
                let digest = Keccak256::digest(deployed_code);
                Bytes32::from_array(digest)
            }
        };
        let observable_bytecode_len = deployed_code.len() as u32;

        let (deployed_code, bytecode_hash, artifacts_len, code_version) = match from_ee {
            ExecutionEnvironmentType::NoEE => {
                return Err(internal_error!("Deployment cannot happen in NoEE").into())
            }
            ExecutionEnvironmentType::EVM => {
                let artifacts = evm_interpreter::BytecodePreprocessingData::create_artifacts(
                    alloc,
                    deployed_code,
                    resources,
                )?;
                let artifacts = artifacts.as_slice();
                let bytecode_hash =
                    Self::compute_bytecode_hash(from_ee, deployed_code, artifacts, resources)?;
                let artifacts_len = artifacts.len() as u32;
                let padding_len = bytecode_padding_len(deployed_code.len());
                let bytecode_len = observable_bytecode_len + (padding_len as u32) + artifacts_len;

                let padding = [0u8; core::mem::size_of::<u64>() - 1];
                let padding = &padding[..padding_len];
                // save bytecode
                let deployed_code = preimages_cache.record_preimage::<PROOF_ENV>(
                    from_ee,
                    &(PreimageRequest {
                        hash: bytecode_hash,
                        expected_preimage_len_in_bytes: bytecode_len,
                        preimage_type: PreimageType::Bytecode,
                    }),
                    resources,
                    &[deployed_code, padding, artifacts],
                )?;
                (
                    deployed_code,
                    bytecode_hash,
                    artifacts_len,
                    evm_interpreter::ARTIFACTS_CACHING_CODE_VERSION_BYTE,
                )
            }
        };

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        account_data.update(|cache_record| {
            cache_record.update(|v, m| {
                v.observable_bytecode_hash = observable_bytecode_hash;
                v.observable_bytecode_len = observable_bytecode_len;
                v.bytecode_hash = bytecode_hash;
                v.unpadded_code_len = observable_bytecode_len;
                v.artifacts_len = artifacts_len;
                v.versioning_data.set_as_deployed();
                v.versioning_data.set_ee_version(from_ee as u8);
                v.versioning_data.set_code_version(code_version);

                m.basic.deployed_in_tx = Some(cur_tx);
                // This is unlikely to happen, this case shouldn't be reachable by higher level logic
                // but just in case if force deployed contract was redeployed with regular deployment we want to publish it
                m.not_publish_bytecode = false;

                Ok(())
            })
        })?;

        Ok((deployed_code, bytecode_hash, observable_bytecode_len))
    }

    /// Assumes [code_hash] is of default version, which does not contain
    /// artifacts cached in the bytecode.
    /// As this storage model caches artifacts, this function decommitts
    /// the code from [code_hash], computes the artifacts and re-hashes
    /// to get the actual [bytecode_hash] for the account.
    pub fn set_bytecode_details<const PROOF_ENV: bool>(
        &mut self,
        resources: &mut R,
        at_address: &B160,
        ee: ExecutionEnvironmentType,
        code_hash: Bytes32,
        unpadded_bytecode_len: u32,
        artifacts_len: u32,
        observable_bytecode_hash: Bytes32,
        observable_bytecode_len: u32,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        let cur_tx = self.current_tx_id;
        let alloc = self.alloc.clone();

        let mut account_data = self.materialize_element::<PROOF_ENV>(
            ee,
            resources,
            at_address,
            storage,
            preimages_cache,
            oracle,
            false,
            true,
        )?;

        Self::charge_account_persist_cost_if_needed(cur_tx, &mut account_data, resources)?;

        let request = PreimageRequest {
            hash: code_hash,
            expected_preimage_len_in_bytes: unpadded_bytecode_len,
            preimage_type: PreimageType::Bytecode,
        };
        let deployed_code =
            preimages_cache.get_preimage::<PROOF_ENV>(ee, &request, resources, oracle)?;

        let (_deployed_code, bytecode_hash, artifacts_len, code_version) = match ee {
            ExecutionEnvironmentType::NoEE => {
                return Err(internal_error!("Deployment cannot happen in NoEE").into())
            }
            ExecutionEnvironmentType::EVM => {
                // For EVM, default code version doesn't cache artifacts
                assert_eq!(artifacts_len, 0);
                let artifacts = evm_interpreter::BytecodePreprocessingData::create_artifacts(
                    alloc,
                    deployed_code,
                    resources,
                )?;
                let artifacts = artifacts.as_slice();
                let bytecode_hash =
                    Self::compute_bytecode_hash(ee, deployed_code, artifacts, resources)?;
                let artifacts_len = artifacts.len() as u32;
                let padding_len = bytecode_padding_len(deployed_code.len());
                let bytecode_len = observable_bytecode_len + (padding_len as u32) + artifacts_len;

                let padding = [0u8; core::mem::size_of::<u64>() - 1];
                let padding = &padding[..padding_len];
                // save bytecode
                let deployed_code = preimages_cache.record_preimage::<PROOF_ENV>(
                    ee,
                    &(PreimageRequest {
                        hash: bytecode_hash,
                        expected_preimage_len_in_bytes: bytecode_len,
                        preimage_type: PreimageType::Bytecode,
                    }),
                    resources,
                    &[deployed_code, padding, artifacts],
                )?;
                (
                    deployed_code,
                    bytecode_hash,
                    artifacts_len,
                    evm_interpreter::ARTIFACTS_CACHING_CODE_VERSION_BYTE,
                )
            }
        };

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        account_data.update(|cache_record| {
            cache_record.update(|v, m| {
                v.observable_bytecode_hash = observable_bytecode_hash;
                v.observable_bytecode_len = observable_bytecode_len;
                v.bytecode_hash = bytecode_hash;
                v.unpadded_code_len = unpadded_bytecode_len;
                v.artifacts_len = artifacts_len;
                v.versioning_data.set_as_deployed();
                v.versioning_data.set_ee_version(ee as u8);
                v.versioning_data.set_code_version(code_version);

                m.basic.deployed_in_tx = Some(cur_tx);
                m.not_publish_bytecode = true;

                Ok(())
            })
        })?;

        Ok(())
    }

    pub fn set_delegation<const PROOF_ENV: bool>(
        &mut self,
        resources: &mut R,
        at_address: &B160,
        delegate: &B160,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        let cur_tx = self.current_tx_id;
        let mut account_data = resources.with_infinite_ergs(|inf_resources| {
            self.materialize_element::<PROOF_ENV>(
                ExecutionEnvironmentType::EVM,
                inf_resources,
                at_address,
                storage,
                preimages_cache,
                oracle,
                false,
                true,
            )
        })?;

        Self::charge_account_persist_cost_if_needed(cur_tx, &mut account_data, resources)?;

        let (
            observable_bytecode_hash,
            observable_bytecode_len,
            bytecode_hash,
            artifacts_len,
            code_version,
            delegated,
        ) = if delegate == &B160::ZERO {
            (Bytes32::ZERO, 0, Bytes32::ZERO, 0, 0u8, false)
        } else {
            // Bytecode is: 0xef0100 || address
            let mut code = [0u8; 23];
            code[0..3].copy_from_slice(&EIP7702_DELEGATION_MARKER);
            code[3..].copy_from_slice(&delegate.to_be_bytes::<{ B160::BYTES }>());

            // compute observable and true hashes of bytecode
            let observable_bytecode_hash = {
                let native_cost = keccak256_native_cost::<R>(code.len());
                resources.charge(&R::from_native(native_cost))?;
                use crypto::sha3::Keccak256;
                use crypto::MiniDigest;
                let digest = Keccak256::digest(code);
                Bytes32::from_array(digest)
            };

            let observable_bytecode_len = code.len() as u32;

            // We compute bytecode hash including padding, for compatibility
            // We set EE type to EVM, just to use Blake in the helper function
            let bytecode_hash =
                Self::compute_bytecode_hash(ExecutionEnvironmentType::EVM, &code, &[], resources)?;
            let artifacts_len = 0;
            let padding_len = bytecode_padding_len(code.len());
            let bytecode_len = observable_bytecode_len + (padding_len as u32) + artifacts_len;
            let padding = [0u8; core::mem::size_of::<u64>() - 1];
            let padding = &padding[..padding_len];
            // save bytecode
            preimages_cache.record_preimage::<PROOF_ENV>(
                ExecutionEnvironmentType::NoEE,
                &(PreimageRequest {
                    hash: bytecode_hash,
                    expected_preimage_len_in_bytes: bytecode_len,
                    preimage_type: PreimageType::Bytecode,
                }),
                resources,
                &[&code, padding, &[]],
            )?;
            (
                observable_bytecode_hash,
                observable_bytecode_len,
                bytecode_hash,
                artifacts_len,
                evm_interpreter::ARTIFACTS_CACHING_CODE_VERSION_BYTE,
                true,
            )
        };

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        account_data.update(|cache_record| {
            cache_record.update(|v, m| {
                v.observable_bytecode_hash = observable_bytecode_hash;
                v.observable_bytecode_len = observable_bytecode_len;
                v.bytecode_hash = bytecode_hash;
                v.unpadded_code_len = observable_bytecode_len;
                v.artifacts_len = artifacts_len;

                if delegated {
                    v.versioning_data.set_as_delegated();
                    // Delegated accounts can only be of EVM EE type.
                    // Note that delegates can be of any EE type, the restriction
                    // is just on the delegated account itself.
                    v.versioning_data
                        .set_ee_version(ExecutionEnvironmentType::EVM_EE_BYTE);
                } else {
                    v.versioning_data.unset_deployment_status();
                    v.versioning_data
                        .set_ee_version(ExecutionEnvironmentType::NO_EE_BYTE);
                }

                v.versioning_data.set_code_version(code_version);

                // This is unlikely to happen, this case shouldn't be reachable by higher level logic
                // but just in case if force deployed contract was redeployed with regular deployment we want to publish it
                m.not_publish_bytecode = false;

                Ok(())
            })
        })?;

        Ok(())
    }

    pub fn mark_for_deconstruction<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: &B160,
        nominal_token_beneficiary: &B160,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
        in_constructor: bool,
    ) -> Result<U256, DeconstructionSubsystemError> {
        let cur_tx = self.current_tx_id;
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            from_ee,
            resources,
            at_address,
            storage,
            preimages_cache,
            oracle,
            true,
            false,
        )?;

        Self::charge_account_persist_cost_if_needed(cur_tx, &mut account_data, resources)?;

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        let same_address = at_address == nominal_token_beneficiary;
        let transfer_amount = account_data.current().value().balance;

        // We consider two cases: either deconstruction happens within the same
        // tx as the address was deployed or it happens in constructor code.
        // Note that the contract is only deployed after finalization of
        // constructor, so in the second case `deployed_in_tx` won't be set
        // yet.
        let should_be_deconstructed = account_data.current().metadata().basic.deployed_in_tx
            == Some(cur_tx)
            || in_constructor;

        if should_be_deconstructed {
            account_data
                .element_properties_mut()
                .mark_value_as_observed();
            account_data.update(|data| {
                data.update_metadata(|metadata| {
                    metadata.basic.is_marked_for_deconstruction = true;

                    Ok(())
                })
            })?;
        }

        // First do the token transfer
        // We do the transfer first to charge for cold access.
        if !same_address {
            self.transfer_nominal_token_value_inner::<PROOF_ENV>(
                from_ee,
                resources,
                at_address,
                nominal_token_beneficiary,
                &transfer_amount,
                storage,
                preimages_cache,
                oracle,
                true,
            )
            .map_err(wrap_error!())?;
        } else if should_be_deconstructed {
            account_data.update(|cache_record| {
                cache_record.update(|v, _| {
                    v.balance = U256::ZERO;
                    Ok(())
                })
            })?;
        }

        // Charge extra gas if positive value to new account
        if !transfer_amount.is_zero() {
            match from_ee {
                ExecutionEnvironmentType::NoEE => (),
                ExecutionEnvironmentType::EVM => {
                    let entry = match self.cache.get(nominal_token_beneficiary.into()) {
                        Some(entry) => Ok(entry),
                        None => Err(internal_error!("Account assumed warm but not in cache")),
                    }?;
                    let beneficiary_properties = entry.current().value();

                    let beneficiary_is_empty = beneficiary_properties.nonce == 0
                        && beneficiary_properties.unpadded_code_len == 0
                        // We need to check with the transferred amount,
                        // this means it was 0 before the transfer.
                        && beneficiary_properties.balance == transfer_amount;
                    if beneficiary_is_empty {
                        use evm_interpreter::gas_constants::NEWACCOUNT;
                        let ergs_to_spend = Ergs(NEWACCOUNT * ERGS_PER_GAS);
                        resources.charge(&R::from_ergs(ergs_to_spend))?;
                    }
                }
            }
        }

        Ok(transfer_amount)
    }

    pub fn finish_tx(
        &mut self,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
    ) -> Result<(), InternalError> {
        // Actually deconstructing accounts
        self.cache.apply_to_last_record_of_pending_changes(
            |key, (_initial, current), cache_appearance| {
                if current.value.metadata().basic.is_marked_for_deconstruction {
                    // NOTE: it can only happen if the account is initially empty,
                    // so we need to make sure that it was observed earlier - when bytecode was deployed
                    assert!(cache_appearance.is_value_observed());
                    current.value.update(|x, metadata| {
                        metadata.basic.is_marked_for_deconstruction = false;
                        *x = AccountProperties::TRIVIAL_VALUE;
                        Ok(())
                    })?;
                    storage
                        .0
                        .clear_state_impl(key)
                        .expect("must clear state for code deconstruction in same TX");
                }
                Ok(())
            },
        )?;

        Ok(())
    }
}

define_subsystem!(AccountCache,
                  interface AccountCacheInterfaceError {},
                  cascade AccountCacheCascadedError {
                      EvmSubsystem(EvmSubsystemError),
                  }
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_implementation::caches::generic_pubdata_aware_plain_storage::GenericPubdataAwarePlainStorage;
    use crate::system_implementation::system::EthereumLikeStorageAccessCostModel;
    use std::alloc::Global;
    use std::mem::size_of;
    use storage_models::common_structs::snapshottable_io::SnapshottableIo;
    use zk_ee::internal_error;
    use zk_ee::memory::stack_implementations::vec_stack::VecStackFactory;
    use zk_ee::oracle::query_ids::INITIAL_STORAGE_SLOT_VALUE_QUERY_ID;
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::oracle::IOOracle;
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
    use zk_ee::storage_types::InitialStorageSlotData;
    use zk_ee::system::errors::internal::InternalError;
    use zk_ee::system::Resource;
    use zk_ee::types_config::EthereumIOTypesConfig;

    type TestResources = BaseResources<DecreasingNative>;
    type TestStorage = NewStorageWithAccountPropertiesUnderHash<
        Global,
        VecStackFactory,
        4,
        TestResources,
        EthereumLikeStorageAccessCostModel,
    >;
    type TestAccountCache = NewModelAccountCache<
        Global,
        TestResources,
        EthereumLikeStorageAccessCostModel,
        VecStackFactory,
        4,
    >;

    struct EmptyAccountOracle;

    impl IOOracle for EmptyAccountOracle {
        type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            match query_type {
                INITIAL_STORAGE_SLOT_VALUE_QUERY_ID => {
                    let response = InitialStorageSlotData::<EthereumIOTypesConfig> {
                        is_new_storage_slot: true,
                        initial_value: Bytes32::ZERO,
                    };
                    let values: Vec<_> = response.iter().collect();
                    Ok(Box::new(values.into_iter()))
                }
                _ => Err(internal_error!("unexpected oracle query in test")),
            }
        }
    }

    struct ExistingAccountOracle {
        account_hash: Bytes32,
        preimage_words: Vec<usize>,
        preimage_queries: usize,
    }

    impl ExistingAccountOracle {
        fn new(account: &AccountProperties) -> Self {
            let encoded = account.encoding();
            let mut padded = encoded.to_vec();
            let word_size = size_of::<usize>();
            padded.resize(encoded.len().div_ceil(word_size) * word_size, 0);
            let preimage_words = padded
                .chunks_exact(word_size)
                .map(|chunk| usize::from_ne_bytes(chunk.try_into().unwrap()))
                .collect();

            Self {
                account_hash: account.compute_hash(),
                preimage_words,
                preimage_queries: 0,
            }
        }
    }

    impl IOOracle for ExistingAccountOracle {
        type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            match query_type {
                INITIAL_STORAGE_SLOT_VALUE_QUERY_ID => {
                    let response = InitialStorageSlotData::<EthereumIOTypesConfig> {
                        is_new_storage_slot: false,
                        initial_value: self.account_hash,
                    };
                    let values: Vec<_> = response.iter().collect();
                    Ok(Box::new(values.into_iter()))
                }
                crate::system_implementation::flat_storage_model::preimage_cache::FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID => {
                    self.preimage_queries += 1;
                    Ok(Box::new(self.preimage_words.clone().into_iter()))
                }
                _ => Err(internal_error!("unexpected oracle query in test")),
            }
        }
    }

    #[test]
    fn noop_balance_update_charges_warm_account_cache_write() {
        let mut storage: TestStorage = NewStorageWithAccountPropertiesUnderHash(
            GenericPubdataAwarePlainStorage::new_from_parts(
                Global,
                EthereumLikeStorageAccessCostModel,
            ),
        );
        let mut preimages_cache =
            BytecodeAndAccountDataPreimagesStorage::<TestResources, Global>::new_from_parts(Global);
        let mut account_cache = TestAccountCache::new_from_parts(Global);
        let mut oracle = EmptyAccountOracle;
        let address = B160::from_limbs([0x1234, 0, 0]);

        storage.begin_new_tx();
        account_cache.begin_new_tx();

        let mut resources = TestResources::FORMAL_INFINITE;
        let initial_native = resources.native().as_u64();
        let initial_retained_bytes = preimages_cache.estimated_retained_bytes();

        let previous_balance = account_cache
            .update_nominal_token_value::<false>(
                ExecutionEnvironmentType::NoEE,
                &mut resources,
                &address,
                |balance| Ok(*balance),
                &mut storage,
                &mut preimages_cache,
                &mut oracle,
                false,
            )
            .expect("no-op update on an empty account should succeed");

        assert_eq!(previous_balance, U256::ZERO);

        let charged_native = initial_native - resources.native().as_u64();
        let expected_native = WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST
            + WARM_STORAGE_READ_NATIVE_COST
            + COLD_NEW_STORAGE_READ_NATIVE_COST
            + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST;

        assert_eq!(
            charged_native, expected_native,
            "no-op balance updates must still pay the warm account-cache write cost"
        );

        let reserved_entry_bytes =
            BytecodeAndAccountDataPreimagesStorage::<TestResources, Global>::estimated_entry_bytes(
                AccountProperties::ENCODED_SIZE,
            )
            .unwrap();
        assert_eq!(
            preimages_cache.estimated_retained_bytes(),
            initial_retained_bytes + reserved_entry_bytes,
            "first materialization must precharge one final account snapshot"
        );

        account_cache
            .touch_account::<false>(
                ExecutionEnvironmentType::NoEE,
                &mut resources,
                &address,
                &mut storage,
                &mut preimages_cache,
                &mut oracle,
            )
            .unwrap();
        assert_eq!(
            preimages_cache.estimated_retained_bytes(),
            initial_retained_bytes + reserved_entry_bytes,
            "an already cached account must not reserve twice"
        );
    }

    #[test]
    fn cached_account_from_invalidated_tx_readmits_its_preimage() {
        let mut storage: TestStorage = NewStorageWithAccountPropertiesUnderHash(
            GenericPubdataAwarePlainStorage::new_from_parts(
                Global,
                EthereumLikeStorageAccessCostModel,
            ),
        );
        let mut preimages_cache =
            BytecodeAndAccountDataPreimagesStorage::<TestResources, Global>::new_from_parts(Global);
        let mut account_cache = TestAccountCache::new_from_parts(Global);
        let account = AccountProperties {
            balance: U256::from(1u64),
            ..AccountProperties::default()
        };
        let mut oracle = ExistingAccountOracle::new(&account);
        let address = B160::from_limbs([0x5678, 0, 0]);
        let estimated_entry_bytes =
            BytecodeAndAccountDataPreimagesStorage::<TestResources, Global>::estimated_entry_bytes(
                AccountProperties::ENCODED_SIZE,
            )
            .unwrap();

        storage.begin_new_tx();
        preimages_cache.begin_new_tx();
        account_cache.begin_new_tx();
        let storage_rollback = storage.start_frame();
        let preimage_rollback = preimages_cache.start_frame();
        let account_rollback = account_cache.start_frame();

        let mut first_resources = TestResources::FORMAL_INFINITE;
        account_cache
            .touch_account::<false>(
                ExecutionEnvironmentType::NoEE,
                &mut first_resources,
                &address,
                &mut storage,
                &mut preimages_cache,
                &mut oracle,
            )
            .unwrap();
        assert_eq!(
            preimages_cache.estimated_bytes_added_in_current_tx(),
            estimated_entry_bytes
        );

        storage.finish_frame(Some(&storage_rollback)).unwrap();
        preimages_cache
            .finish_frame(Some(&preimage_rollback))
            .unwrap();
        account_cache.finish_frame(Some(&account_rollback)).unwrap();
        let retained_bytes = preimages_cache.estimated_retained_bytes();
        assert!(
            !account_cache
                .cache
                .get((&address).into())
                .unwrap()
                .current()
                .metadata()
                .initial_preimage_admitted
        );

        // Do not finish the first candidate: it was dropped. Its initial
        // account-cache record and raw preimage survive, but neither is part of
        // the proving run for the next candidate.
        storage.begin_new_tx();
        preimages_cache.begin_new_tx();
        account_cache.begin_new_tx();
        assert_eq!(preimages_cache.estimated_bytes_added_in_current_tx(), 0);
        let tx_limit = crate::system_implementation::flat_storage_model::preimage_cache::MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX;
        let over_limit_bytes = tx_limit - estimated_entry_bytes + 1;
        preimages_cache.set_estimated_bytes_added_in_current_tx(over_limit_bytes);

        let mut second_resources = TestResources::FORMAL_INFINITE;
        assert!(account_cache
            .touch_account::<false>(
                ExecutionEnvironmentType::NoEE,
                &mut second_resources,
                &address,
                &mut storage,
                &mut preimages_cache,
                &mut oracle,
            )
            .is_err());

        assert_eq!(
            oracle.preimage_queries, 1,
            "decoded account should be reused"
        );
        assert!(preimages_cache.tx_limit_hit_for_current_tx());
        assert!(!preimages_cache.block_limit_hit_for_current_tx());
        assert_eq!(
            preimages_cache.estimated_bytes_added_in_current_tx(),
            over_limit_bytes,
            "failed re-admission must not partially update the counter"
        );
        assert_eq!(preimages_cache.estimated_retained_bytes(), retained_bytes);

        // At the exact boundary, the same account-cache hit must succeed and
        // consume all remaining transaction-local budget without querying the
        // oracle or retaining another physical entry.
        storage.begin_new_tx();
        preimages_cache.begin_new_tx();
        account_cache.begin_new_tx();
        preimages_cache.set_estimated_bytes_added_in_current_tx(tx_limit - estimated_entry_bytes);

        let mut third_resources = TestResources::FORMAL_INFINITE;
        account_cache
            .touch_account::<false>(
                ExecutionEnvironmentType::NoEE,
                &mut third_resources,
                &address,
                &mut storage,
                &mut preimages_cache,
                &mut oracle,
            )
            .unwrap();

        assert_eq!(
            preimages_cache.estimated_bytes_added_in_current_tx(),
            tx_limit,
            "the surviving account cache must not bypass preimage admission"
        );
        assert_eq!(oracle.preimage_queries, 1);
        assert_eq!(preimages_cache.estimated_retained_bytes(), retained_bytes);
        assert!(
            account_cache
                .cache
                .get((&address).into())
                .unwrap()
                .current()
                .metadata()
                .initial_preimage_admitted
        );

        account_cache.finish_tx(&mut storage).unwrap();
        storage.finish_tx().unwrap();
        preimages_cache.finish_tx().unwrap();
        storage.begin_new_tx();
        preimages_cache.begin_new_tx();
        account_cache.begin_new_tx();

        let mut fourth_resources = TestResources::FORMAL_INFINITE;
        account_cache
            .touch_account::<false>(
                ExecutionEnvironmentType::NoEE,
                &mut fourth_resources,
                &address,
                &mut storage,
                &mut preimages_cache,
                &mut oracle,
            )
            .unwrap();
        assert_eq!(
            preimages_cache.estimated_bytes_added_in_current_tx(),
            0,
            "an accepted account preimage must not be charged again"
        );
        assert_eq!(oracle.preimage_queries, 1);
    }
}
