//! Account cache, backed by a history map.
//! This caches the actual account data, which will
//! then be published into the preimage storage.
use super::super::cost_constants::*;
use crate::cost_constants::blake2s_native_cost;
use crate::system_functions::keccak256::keccak256_native_cost;
use crate::system_implementation::caches::basic_account_properties::BasicAccountPropertiesMetadata;
use crate::system_implementation::caches::cache_element_properties::CacheElementProperties;
use crate::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use crate::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountProperties;
use crate::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountPropertiesQuery;
use crate::system_implementation::ethereum_storage_model::caches::full_storage_cache::EthereumStorageCache;
use crate::system_implementation::ethereum_storage_model::caches::preimage::BytecodeKeccakPreimagesStorage;
use crate::system_implementation::ethereum_storage_model::caches::preimage::PreimageRequestForUnknownLength;
use crate::system_implementation::ethereum_storage_model::caches::EMPTY_STRING_KECCAK_HASH;
use crate::system_implementation::ethereum_storage_model::interner::{
    Interned, MAX_UNIQUE_ADDRESSES,
};
use crate::system_implementation::ethereum_storage_model::EMPTY_ROOT_HASH;
use core::alloc::Allocator;
use core::marker::PhantomData;
use evm_interpreter::errors::EvmSubsystemError;
use ruint::aliases::B160;
use ruint::aliases::U256;
use storage_models::common_structs::PreimageCacheModel;
use zk_ee::common_structs::cache_record::CacheRecord;
use zk_ee::common_structs::history_map::CacheSnapshotId;
use zk_ee::common_structs::history_map::DenseIndex;
use zk_ee::common_structs::history_map::ElementHandle;
use zk_ee::common_structs::history_map::HistoryMap;
use zk_ee::common_structs::history_map::HistoryMapItemRefMut;
use zk_ee::common_structs::PreimageType;
use zk_ee::define_subsystem;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::interface_error;
use zk_ee::internal_error;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::memory_io::OracleQuery;
use zk_ee::oracle::IOOracle;
use zk_ee::system::BalanceSubsystemError;
use zk_ee::system::Computational;
use zk_ee::system::DeconstructionSubsystemError;
use zk_ee::system::NonceError;
use zk_ee::system::NonceSubsystemError;
use zk_ee::utils::Bytes32;
use zk_ee::wrap_error;
use zk_ee::{
    system::{
        errors::{internal::InternalError, system::SystemError},
        AccountData, AccountDataRequest, Maybe, Resources,
    },
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
};

pub type AccountRecord = CacheRecord<EthereumAccountProperties, BasicAccountPropertiesMetadata>;

pub type AddressItem<'a, A> =
    HistoryMapItemRefMut<'a, u32, AccountRecord, A, CacheElementProperties>;

pub type AccountHandle<A> = ElementHandle<u32, AccountRecord, A, CacheElementProperties>;

/// Accounts are keyed by their interned address index, so the map's index is a
/// direct table: a lookup is one load.
pub type AccountCacheMap<A> =
    HistoryMap<u32, AccountRecord, A, CacheElementProperties, DenseIndex<AccountHandle<A>, A>>;

pub struct EthereumAccountCache<
    A: Allocator + Clone, // = Global,
    R: Resources,
    SF: StackFactory<N>,
    const N: usize,
> {
    pub(crate) cache: AccountCacheMap<A>,
    pub(crate) current_tx_number: u32,
    #[allow(dead_code)]
    alloc: A,
    phantom: PhantomData<(R, SF)>,
}

impl<A: Allocator + Clone, R: Resources, SF: StackFactory<N>, const N: usize>
    EthereumAccountCache<A, R, SF, N>
{
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            cache: HistoryMap::with_index(
                DenseIndex::new_in(MAX_UNIQUE_ADDRESSES, allocator.clone()),
                allocator.clone(),
            ),
            current_tx_number: 0,
            alloc: allocator.clone(),
            phantom: PhantomData,
        }
    }

    /// Read element and initialize it if needed
    fn materialize_element<const PROOF_ENV: bool>(
        &'_ mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
        observe: bool,
    ) -> Result<AddressItem<'_, A>, SystemError> {
        self.materialize_element_ext::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            oracle,
            is_selfdestruct,
            observe,
            true,
        )
    }

    /// Warms the element up, charging for the access, and loads its value from
    /// the oracle if `load_value` is set and it was not loaded yet.
    ///
    /// Without `load_value` (a touch: access list, precompile warm-up) a missing
    /// element is inserted with an undefined value: no oracle IO happens and no proof obligation is
    /// created, since Ethereum does not load such accounts either. Loading fills
    /// every history record in place, as no write can precede it, so a rollback
    /// keeps the loaded value and only reverts the warmth.
    #[allow(clippy::too_many_arguments)]
    fn materialize_element_ext<const PROOF_ENV: bool>(
        &'_ mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
        observe: bool,
        load_value: bool,
    ) -> Result<AddressItem<'_, A>, SystemError> {
        debug_assert!(load_value || !observe, "observing needs the value");
        let gas = match ee_type {
            ExecutionEnvironmentType::NoEE => 0,
            ExecutionEnvironmentType::EVM =>
            // For selfdestruct, there's no warm access cost
            {
                if is_selfdestruct {
                    0
                } else {
                    WARM_PROPERTIES_ACCESS_COST_GAS
                }
            }
        };
        resources.charge_legacy_gas_and_native(gas, WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST)?;

        let current_tx_number = self.current_tx_number;
        let mut x = self
            .cache
            .get_or_insert::<SystemError>(&address.index, || {
                // Undefined: no value declared yet, it is loaded below when it is needed.
                // Note: we initialize it as cold, should be warmed up separately
                // Since in case of revert it should become cold again and initial record can't be rolled back
                Ok((
                    CacheRecord::new(EthereumAccountProperties::EMPTY_ACCOUNT),
                    CacheElementProperties::undefined(),
                ))
            })?;

        // Warm up element according to EVM rules if needed
        let is_warm = x.current().metadata().considered_warm(current_tx_number);
        if is_warm == false {
            match ee_type {
                ExecutionEnvironmentType::NoEE => {}
                ExecutionEnvironmentType::EVM => {
                    let mut cost: R = if evm_interpreter::utils::is_precompile(address.key) {
                        R::empty() // We've charged the access already.
                    } else {
                        R::from_legacy_gas_saturating(COLD_PROPERTIES_ACCESS_EXTRA_COST_GAS)
                    };
                    if is_selfdestruct {
                        // Selfdestruct doesn't charge for warm, but it
                        // includes the warm cost for cold access
                        cost.add_legacy_gas(WARM_PROPERTIES_ACCESS_COST_GAS)
                    };
                    resources.charge(&cost)?;
                }
            }
            // mark as warm
            x.update(|cache_record| {
                cache_record.update_metadata(|m| {
                    assert!(m.is_marked_for_deconstruction == false); // any deconstuction should finish in previous TX
                    m.last_touched_in_tx = Some(current_tx_number);
                    Ok(())
                })
            })?;
        }

        if load_value && !x.element_properties().is_value_declared() {
            // we just ask the oracle for properties
            let acc_data = EthereumAccountPropertiesQuery::get(oracle, address.key)?;
            let empty_account = acc_data.is_empty();
            x.for_each_record_mut(|record| {
                record
                    .update(|v, _| {
                        *v = acc_data;
                        Ok(())
                    })
                    .expect("filling in the loaded value can not fail");
            });
            x.element_properties_mut()
                .mark_value_as_declared(empty_account);
        }

        // appearance mark
        if observe {
            x.element_properties_mut().mark_value_as_observed()?;
        }

        Ok(x)
    }

    fn update_nominal_token_value_inner<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        update_fn: impl FnOnce(&U256) -> Result<U256, BalanceSubsystemError>,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
    ) -> Result<U256, BalanceSubsystemError> {
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            oracle,
            is_selfdestruct,
            false,
        )?;

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        let cur = account_data.current().value().balance;
        let new = update_fn(&cur)?;
        account_data
            .element_properties_mut()
            .mark_value_as_observed()?;
        account_data.update(|cache_record| {
            cache_record.update(|v, _| {
                v.balance = new;
                Ok(())
            })
        })?;

        Ok(cur)
    }

    fn transfer_nominal_token_value_inner<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        from: Interned<'_, B160>,
        to: Interned<'_, B160>,
        amount: &U256,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
    ) -> Result<(), BalanceSubsystemError> {
        use zk_ee::system::BalanceError;

        let mut f = |addr: Interned<'_, B160>, op: fn(U256, U256) -> (U256, bool), err| {
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
                oracle,
                is_selfdestruct,
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

    pub fn calculate_pubdata_used_by_tx(&self) -> u32 {
        0
    }

    pub fn begin_new_tx(&mut self) {
        self.cache.commit();

        self.current_tx_number += 1;
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

    /// `address` is the interned index of the address if it was interned: an
    /// address that was never interned can not be in the cache.
    pub fn read_account_balance_assuming_warm(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Option<u32>,
    ) -> Result<<EthereumIOTypesConfig as SystemIOTypesConfig>::NominalTokenValue, SystemError>
    {
        // Charge for gas
        match ee_type {
            ExecutionEnvironmentType::NoEE => (),
            ExecutionEnvironmentType::EVM => {
                resources.charge_legacy_gas(KNOWN_TO_BE_WARM_PROPERTIES_ACCESS_COST_GAS)?
            }
        }

        match address.and_then(|index| self.cache.get(&index)) {
            Some(cache_item) if cache_item.key_properties().is_value_declared() => {
                Ok(cache_item.current().value().balance)
            }
            Some(_) => {
                Err(internal_error!("Balance assumed warm but its value is undefined").into())
            }
            None => Err(internal_error!("Balance assumed warm but not in cache").into()),
        }
    }

    pub fn touch_account<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        oracle: &mut impl IOOracle,
        observe: bool,
    ) -> Result<(), SystemError> {
        // a plain touch does not load the account: see `materialize_element_ext`
        self.materialize_element_ext::<PROOF_ENV>(
            ee_type, resources, address, oracle, false, observe, observe,
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
        address: Interned<'_, B160>,
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
        preimages_cache: &mut BytecodeKeccakPreimagesStorage<R, A>,
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
        let mut account_data = self
            .materialize_element::<PROOF_ENV>(ee_type, resources, address, oracle, false, true)?;
        // we are actually going to use account properties, so we should mark it so
        account_data
            .element_properties_mut()
            .mark_value_as_observed()?;
        let element_properties = account_data.element_properties();
        let full_data = account_data.current().value();

        // we already charged for "cold" case, and now can charge more precisely

        // NOTE: we didn't yet decommit the bytecode, BUT charged for it (all properties are warm at
        // once or not), so if we do not access it ever we will not need to pollute preimages cache

        let bytecode_hash_is_zero = full_data.bytecode_hash.is_zero();

        let needs_preimage = ObservableBytecodeLen::IS_MATERIAL
            || BytecodeLen::IS_MATERIAL
            || ArtifactsLen::IS_MATERIAL
            || Bytecode::IS_MATERIAL
            || IsDelegated::IS_MATERIAL;
        // The jumpdest artifacts are only for the code that is about to run, and that request
        // asks for their length. Observing the code (`EXTCODESIZE`, `EXTCODECOPY`, the delegation
        // check) loads it, as its length and bytes must be verified against the hash, but does
        // not preprocess it.
        let needs_artifacts = ArtifactsLen::IS_MATERIAL;
        // (bytecode in the form for execution, length of the code itself, length of the artifacts)
        let (bytecode, code_length, artifacts_len) = if needs_preimage {
            // NOTE: deconstruction happens at the end of the TX, so even deconstructed accounts would NOT
            // respond with empty bytecode (well, WTF)

            if bytecode_hash_is_zero {
                debug_assert!(element_properties.is_new_element());

                let res: &'static [u8] = &[];

                (res, 0, 0)
            } else if full_data.bytecode_hash == EMPTY_STRING_KECCAK_HASH {
                let res: &'static [u8] = &[];

                (res, 0, 0)
            } else {
                // can try to get preimage. It comes with the jumpdest artifacts, that are made
                // once per code, and not once per call frame
                let executable = preimages_cache.get_executable_bytecode::<PROOF_ENV>(
                    ee_type,
                    &full_data.bytecode_hash,
                    resources,
                    oracle,
                    needs_artifacts,
                )?;
                (
                    executable.bytecode,
                    executable.code_len,
                    executable.artifacts_len,
                )
            }
        } else {
            (&[][..], 0, 0)
        };

        // artifacts are there only if the code is, and they were requested
        let code_version = if artifacts_len > 0 {
            evm_interpreter::ARTIFACTS_FROM_CODE_CACHE_CODE_VERSION_BYTE
        } else {
            evm_interpreter::DEFAULT_CODE_VERSION_BYTE
        };

        let is_delegated = if code_length == 3 + 20 {
            bytecode[..3] == zk_ee::system::EIP7702_DELEGATION_MARKER
        } else {
            false
        };

        Ok(AccountData {
            ee_version: Maybe::construct(|| ExecutionEnvironmentType::EVM as u8),
            observable_bytecode_hash: Maybe::construct(|| full_data.bytecode_hash),
            observable_bytecode_len: Maybe::construct(|| code_length),
            nonce: Maybe::construct(|| full_data.nonce),
            bytecode_hash: Maybe::construct(|| full_data.bytecode_hash),
            unpadded_code_len: Maybe::construct(|| code_length),
            artifacts_len: Maybe::construct(|| artifacts_len),
            nominal_token_balance: Maybe::construct(|| full_data.balance),
            bytecode: Maybe::construct(|| bytecode),
            code_version: Maybe::construct(|| code_version),
            is_delegated: Maybe::construct(|| is_delegated),
        })
    }

    pub fn increment_nonce<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: Interned<'_, B160>,
        increment_by: u64,
        oracle: &mut impl IOOracle,
    ) -> Result<u64, NonceSubsystemError> {
        let mut account_data = self
            .materialize_element::<PROOF_ENV>(ee_type, resources, address, oracle, false, false)?;

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        let nonce = account_data.current().value().nonce;
        if let Some(new_nonce) = nonce.checked_add(increment_by) {
            account_data
                .element_properties_mut()
                .mark_value_as_observed()?;
            account_data.update(|cache_record| {
                cache_record.update(|x, _| {
                    if x.bytecode_hash.is_zero() {
                        x.bytecode_hash = EMPTY_STRING_KECCAK_HASH;
                    }
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
        address: Interned<'_, B160>,
        update_fn: impl FnOnce(&U256) -> Result<U256, BalanceSubsystemError>,
        oracle: &mut impl IOOracle,
    ) -> Result<U256, BalanceSubsystemError> {
        self.update_nominal_token_value_inner::<PROOF_ENV>(
            ee_type, resources, address, update_fn, oracle, false,
        )
    }

    pub fn transfer_nominal_token_value<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        from: Interned<'_, B160>,
        to: Interned<'_, B160>,
        amount: &U256,
        oracle: &mut impl IOOracle,
    ) -> Result<(), BalanceSubsystemError> {
        self.transfer_nominal_token_value_inner::<PROOF_ENV>(
            from_ee, resources, from, to, amount, oracle, false,
        )
    }

    fn compute_bytecode_hash(
        from_ee: ExecutionEnvironmentType,
        observable_bytecode: &[u8],
        resources: &mut R,
    ) -> Result<Bytes32, SystemError> {
        match from_ee {
            ExecutionEnvironmentType::NoEE => {
                Err(internal_error!("Deployment cannot happen in NoEE").into())
            }
            ExecutionEnvironmentType::EVM => {
                use crypto::sha3::Keccak256;
                use crypto::MiniDigest;
                let preimage_len = observable_bytecode.len();
                let native_cost = blake2s_native_cost(preimage_len);
                resources.charge(&R::from_native(R::Native::from_computational(native_cost)))?;

                Ok(Bytes32::from_array(*Keccak256::digest(observable_bytecode)))
            }
        }
    }

    pub fn deploy_code<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: Interned<'_, B160>,
        deployed_code: &[u8],
        preimages_cache: &mut BytecodeKeccakPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(&'static [u8], Bytes32, u32), SystemError> {
        // Charge for code deposit cost
        match from_ee {
            ExecutionEnvironmentType::NoEE => (),
            ExecutionEnvironmentType::EVM => {
                use evm_interpreter::gas_constants::CODEDEPOSIT;
                let code_deposit_cost = CODEDEPOSIT.saturating_mul(deployed_code.len() as u64);
                resources.charge_legacy_gas(code_deposit_cost)?;
            }
        }

        // we charged for everything, and so all IO below will use infinite ergs
        // We've checked that this account is empty in `prepare_for_deployment`.

        let cur_tx = self.current_tx_number;

        let mut account_data = resources.with_infinite_ergs(|inf_resources| {
            self.materialize_element::<PROOF_ENV>(
                from_ee,
                inf_resources,
                at_address,
                oracle,
                false,
                false,
            )
        })?;

        let (deployed_code, bytecode_hash) = match from_ee {
            ExecutionEnvironmentType::NoEE => {
                return Err(internal_error!("Deployment cannot happen in NoEE").into());
            }
            ExecutionEnvironmentType::EVM => {
                let native_cost = keccak256_native_cost::<R>(deployed_code.len());
                resources.charge(&R::from_native(native_cost))?;
                let bytecode_hash = Self::compute_bytecode_hash(from_ee, deployed_code, resources)?;

                // save bytecode
                let deployed_code = preimages_cache.record_preimage::<PROOF_ENV>(
                    from_ee,
                    &(PreimageRequestForUnknownLength {
                        hash: bytecode_hash,
                        preimage_type: PreimageType::Bytecode,
                    }),
                    resources,
                    &[deployed_code],
                )?;
                (deployed_code, bytecode_hash)
            }
        };

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        account_data
            .element_properties_mut()
            .mark_value_as_observed()?;
        account_data.update(|cache_record| {
            cache_record.update(|v, m| {
                v.bytecode_hash = bytecode_hash;

                m.deployed_in_tx = Some(cur_tx);

                Ok(())
            })
        })?;

        Ok((deployed_code, bytecode_hash, deployed_code.len() as u32))
    }

    pub fn mark_for_deconstruction<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: Interned<'_, B160>,
        nominal_token_beneficiary: Interned<'_, B160>,
        oracle: &mut impl IOOracle,
    ) -> Result<U256, DeconstructionSubsystemError> {
        let cur_tx = self.current_tx_number;
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            from_ee, resources, at_address, oracle, true, false,
        )?;
        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        // interned by the same interner: equal indices are equal addresses
        let same_address = at_address.index == nominal_token_beneficiary.index;
        let transfer_amount = account_data.current().value().balance;

        // We consider two cases: either deconstruction happens within the same
        // tx as the address was deployed or it happens in constructor code.
        // Note that the contract is only deployed after finalization of
        // constructor, so in the second case `deployed_in_tx` won't be set
        // yet.
        // We identify if the call happens within a constructor by checking the bytecode.
        // If it's empty, then the call must be in a constructor.
        let in_constructor = account_data.current().value().has_empty_bytecode();
        let should_be_deconstructed =
            account_data.current().metadata().deployed_in_tx == Some(cur_tx) || in_constructor;

        if should_be_deconstructed {
            account_data
                .element_properties_mut()
                .mark_value_as_observed()?;
            account_data.update(|data| {
                data.update_metadata(|metadata| {
                    metadata.is_marked_for_deconstruction = true;

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
                    let entry = match self.cache.get(&nominal_token_beneficiary.index) {
                        Some(entry) => Ok(entry),
                        None => Err(internal_error!("Account assumed warm but not in cache")),
                    }?;
                    let beneficiary_properties = entry.current().value();

                    let beneficiary_is_empty = beneficiary_properties.is_empty_modulo_balance()
                        // We need to check with the transferred amount,
                        // this means it was 0 before the transfer.
                        && beneficiary_properties.balance == transfer_amount;
                    if beneficiary_is_empty {
                        use evm_interpreter::gas_constants::NEWACCOUNT;
                        resources.charge_legacy_gas(NEWACCOUNT)?;
                    }
                }
            }
        }

        Ok(transfer_amount)
    }

    pub fn set_delegation<const PROOF_ENV: bool>(
        &mut self,
        resources: &mut R,
        at_address: Interned<'_, B160>,
        delegate: &B160,
        preimages_cache: &mut BytecodeKeccakPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        let mut account_data = resources.with_infinite_ergs(|inf_resources| {
            self.materialize_element::<PROOF_ENV>(
                ExecutionEnvironmentType::EVM,
                inf_resources,
                at_address,
                oracle,
                false,
                false,
            )
        })?;

        let (bytecode_hash, _bytecode_len, _delegated) = if delegate == &B160::ZERO {
            (EMPTY_STRING_KECCAK_HASH, 0, false)
        } else {
            use zk_ee::system::EIP7702_DELEGATION_MARKER;

            // Bytecode is: 0xef0100 || address
            let mut code = [0u8; 23];
            code[0..3].copy_from_slice(&EIP7702_DELEGATION_MARKER);
            code[3..].copy_from_slice(&delegate.to_be_bytes::<{ B160::BYTES }>());

            // We compute bytecode hash including padding, for compatibility
            // We set EE type to EVM, just to use Blake in the helper function
            let bytecode_hash =
                Self::compute_bytecode_hash(ExecutionEnvironmentType::EVM, &code, resources)?;
            // save bytecode
            preimages_cache.record_preimage::<PROOF_ENV>(
                ExecutionEnvironmentType::NoEE,
                &(PreimageRequestForUnknownLength {
                    hash: bytecode_hash,
                    preimage_type: PreimageType::Bytecode,
                }),
                resources,
                &[&code],
            )?;
            (bytecode_hash, 23, true)
        };

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        account_data
            .element_properties_mut()
            .mark_value_as_observed()?;
        account_data.update(|cache_record| {
            cache_record.update(|v, _m| {
                v.bytecode_hash = bytecode_hash;

                Ok(())
            })
        })?;

        Ok(())
    }

    pub fn finish_tx<P: StorageAccessPolicy<R, Bytes32>>(
        &mut self,
        storage: &mut EthereumStorageCache<A, SF, N, R, P>,
    ) -> Result<(), InternalError> {
        // Actually deconstructing accounts
        self.cache.apply_to_last_record_of_pending_changes(
            |key, (initial, current), cache_appearance| {
                if current.value.metadata().is_marked_for_deconstruction {
                    // NOTE: initially account had 0 nonce, but it could be "material",
                    // with state root being empty, and bytecode hash being hash of empty string.

                    // NOTE: Balance will be zeroed out if deconstruction happens here
                    let initially_empty = cache_appearance.is_new_element();
                    assert!(cache_appearance.is_value_observed());
                    current.value.update(|x, metadata| {
                        metadata.is_marked_for_deconstruction = false;
                        if initially_empty {
                            debug_assert_eq!(
                                initial.value.value(),
                                &EthereumAccountProperties::EMPTY_ACCOUNT
                            );
                            x.balance = U256::ZERO;
                            x.bytecode_hash = Bytes32::ZERO;
                            x.nonce = 0u64;
                        } else {
                            //
                            debug_assert_eq!(initial.value.value().nonce, 0);
                            debug_assert_eq!(
                                initial.value.value().bytecode_hash,
                                EMPTY_STRING_KECCAK_HASH
                            );
                            debug_assert_eq!(initial.value.value().storage_root, EMPTY_ROOT_HASH);
                            x.balance = U256::ZERO;
                            x.bytecode_hash = EMPTY_STRING_KECCAK_HASH;
                            x.nonce = 0u64;
                        }

                        Ok(())
                    })?;
                    storage
                        .clear_state_impl(*key)
                        .expect("must clear state for code deconstruction in same TX");
                }
                Ok(())
            },
        )?;

        Ok(())
    }

    ///
    /// Returns accounts that were changed during execution. `addresses` are the
    /// interned addresses, by index.
    ///
    pub fn net_diffs_iter<'a>(
        &'a self,
        addresses: &'a [B160],
    ) -> impl Iterator<Item = (B160, (u64, U256, Bytes32))> + use<'a, A, SF, N, R> {
        self.cache
            .iter()
            .filter(|v| v.initial().value() != v.current().value())
            .map(|v| {
                let address = addresses[*v.key() as usize];
                let current = v.current().value();
                (
                    address,
                    (current.nonce, current.balance, current.bytecode_hash),
                )
            })
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
    use crate::system_implementation::ethereum_storage_model::caches::account_properties::ETHEREUM_ACCOUNT_INITIAL_STATE_QUERY_ID;
    use crate::system_implementation::ethereum_storage_model::interner::EthereumKeyInterners;
    use crate::system_implementation::system::EthereumLikeStorageAccessCostModel;
    use std::alloc::Global;
    use storage_models::common_structs::snapshottable_io::SnapshottableIo;
    use zk_ee::memory::stack_implementations::vec_stack::VecStackFactory;
    use zk_ee::oracle::memory_io::host::{ResponseBuffer, WriteQueryOutput};
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
    use zk_ee::system::Resource;

    type TestResources = BaseResources<DecreasingNative>;
    type TestAccountCache = EthereumAccountCache<Global, TestResources, VecStackFactory, 4>;
    type TestStorage = EthereumStorageCache<
        Global,
        VecStackFactory,
        4,
        TestResources,
        EthereumLikeStorageAccessCostModel,
    >;

    /// Every account is a funded EOA: it exists in the trie with the empty-string
    /// code hash, zero nonce and an empty storage trie. Counts the queries.
    #[derive(Default)]
    struct FundedAccountsOracle {
        queries: usize,
        response: ResponseBuffer,
    }

    impl zk_ee::oracle::memory_io::MemoryOracle for FundedAccountsOracle {
        fn send_query(&mut self, query_id: u32, _input_word: usize) -> Result<(), InternalError> {
            assert_eq!(query_id, ETHEREUM_ACCOUNT_INITIAL_STATE_QUERY_ID);
            self.queries += 1;
            let account = EthereumAccountProperties {
                balance: U256::from(1_000u64),
                ..EthereumAccountProperties::EMPTY_BUT_EXISTING_ACCOUNT
            };
            let mut response = Vec::new();
            account.write_output(&mut response);
            self.response.set(response)
        }

        zk_ee::memory_oracle_response_methods!(response);
    }

    impl IOOracle for FundedAccountsOracle {
        type RawIterator<'a> = core::iter::Empty<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            panic!("unexpected iterator-based query 0x{query_type:08x}")
        }
    }

    /// A contract created at a pre-funded address whose init code selfdestructs:
    /// the account was created in this transaction, so EIP-6780 deletes it. The
    /// funded address carries the empty-string code hash rather than the
    /// zero-hash convention of a missing account, and the constructor must still
    /// be recognised.
    #[test]
    fn constructor_selfdestruct_on_funded_address_deconstructs_the_account() {
        let mut storage = TestStorage::new_from_parts(Global, EthereumLikeStorageAccessCostModel);
        let mut account_cache = TestAccountCache::new_from_parts(Global);
        let mut interners = EthereumKeyInterners::new_in(Global);
        let mut oracle = FundedAccountsOracle::default();
        let deployee_address = B160::from_limbs([0xdead, 0, 0]);
        let beneficiary_address = B160::from_limbs([0xbeef, 0, 0]);
        let deployee = interners.intern_address(&deployee_address).unwrap();
        let beneficiary = interners.intern_address(&beneficiary_address).unwrap();

        storage.begin_new_tx();
        account_cache.begin_new_tx();
        let mut resources = TestResources::FORMAL_INFINITE;

        // creation sets the nonce before the init code runs (EIP-161)
        account_cache
            .increment_nonce::<false>(
                ExecutionEnvironmentType::EVM,
                &mut resources,
                deployee,
                1,
                &mut oracle,
            )
            .expect("nonce bump at creation");

        let transferred = account_cache
            .mark_for_deconstruction::<false>(
                ExecutionEnvironmentType::EVM,
                &mut resources,
                deployee,
                beneficiary,
                &mut oracle,
            )
            .expect("selfdestruct in the constructor");
        assert_eq!(transferred, U256::from(1_000u64));
        assert!(
            account_cache
                .cache
                .get(&deployee.index)
                .unwrap()
                .current()
                .metadata()
                .is_marked_for_deconstruction,
            "a selfdestruct while the account has no code happens in its constructor"
        );

        account_cache
            .finish_tx(&mut storage)
            .expect("deconstruction at the end of the transaction");

        let final_state = *account_cache
            .cache
            .get(&deployee.index)
            .unwrap()
            .current()
            .value();
        assert_eq!(
            final_state,
            EthereumAccountProperties::EMPTY_BUT_EXISTING_ACCOUNT,
            "the account must be deleted, not left with the creation nonce"
        );
    }

    /// A touch (access list, precompile warm-up) warms the account without
    /// loading it, so a witness may omit accounts the block never uses. The
    /// first access that needs the value loads it, also when the touch happened
    /// in a frame that was rolled back.
    #[test]
    fn touch_does_not_load_the_account_and_a_later_read_does() {
        let mut account_cache = TestAccountCache::new_from_parts(Global);
        let mut preimages =
            BytecodeKeccakPreimagesStorage::<TestResources, Global>::new_from_parts(Global);
        let mut interners = EthereumKeyInterners::new_in(Global);
        let mut oracle = FundedAccountsOracle::default();
        let precompile_address = B160::from_limbs([0x03, 0, 0]);
        let listed_address = B160::from_limbs([0xabcd, 0, 0]);
        let precompile = interners.intern_address(&precompile_address).unwrap();
        let listed = interners.intern_address(&listed_address).unwrap();

        account_cache.begin_new_tx();
        let mut resources = TestResources::FORMAL_INFINITE;

        for address in [precompile, listed] {
            account_cache
                .touch_account::<false>(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    address,
                    &mut oracle,
                    false,
                )
                .expect("touch");
        }
        assert_eq!(oracle.queries, 0, "a touch must not read the account");
        for address in [precompile, listed] {
            let item = account_cache.cache.get(&address.index).unwrap();
            assert!(!item.key_properties().is_value_declared());
            assert!(!item.key_properties().is_value_observed());
            assert!(item
                .current()
                .metadata()
                .considered_warm(account_cache.current_tx_number));
        }
        assert!(
            account_cache
                .read_account_balance_assuming_warm(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    Some(precompile.index)
                )
                .is_err(),
            "an undefined account has no value to hand out"
        );

        // the touch of `listed` is reverted, the one of `precompile` stays
        let snapshot = account_cache.start_frame();
        account_cache
            .touch_account::<false>(
                ExecutionEnvironmentType::NoEE,
                &mut resources,
                listed,
                &mut oracle,
                false,
            )
            .expect("touch");
        account_cache
            .finish_frame(Some(&snapshot))
            .expect("rollback");

        let data = account_cache
            .read_account_properties::<false, _, _, _, _, _, _, _, _, _, _, _>(
                ExecutionEnvironmentType::EVM,
                &mut resources,
                listed,
                AccountDataRequest::empty()
                    .with_nonce()
                    .with_nominal_token_balance(),
                &mut preimages,
                &mut oracle,
            )
            .expect("read");
        assert_eq!(oracle.queries, 1, "the first real access loads the account");
        assert_eq!(data.nominal_token_balance.0, U256::from(1_000u64));
        assert_eq!(data.nonce.0, 0);
        let item = account_cache.cache.get(&listed.index).unwrap();
        assert!(
            item.key_properties().is_value_declared() && item.key_properties().is_value_observed()
        );
        assert!(!item.key_properties().is_new_element());
        assert_eq!(
            item.initial().value().balance,
            U256::from(1_000u64),
            "loading fills the initial record too"
        );

        account_cache
            .read_account_properties::<false, _, _, _, _, _, _, _, _, _, _, _>(
                ExecutionEnvironmentType::EVM,
                &mut resources,
                listed,
                AccountDataRequest::empty().with_nonce(),
                &mut preimages,
                &mut oracle,
            )
            .expect("read again");
        assert_eq!(oracle.queries, 1);
    }
}
