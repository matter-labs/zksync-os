//! Account cache, backed by a history map.
//! This caches the actual account data, which will
//! then be published into the preimage storage.
use super::super::cost_constants::*;
use crate::system_functions::keccak256::keccak256_native_cost;
use crate::system_implementation::cache_structs::storage_values::StorageAccessPolicy;
use crate::system_implementation::cache_structs::AccountPropertiesMetadataNoPubdata;
use crate::system_implementation::cache_structs::BitsOrd160;
use crate::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountProperties;
use crate::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountPropertiesQuery;
use crate::system_implementation::ethereum_storage_model::caches::full_storage_cache::EthereumStorageCache;
use crate::system_implementation::ethereum_storage_model::caches::preimage::BytecodeKeccakPreimagesStorage;
use crate::system_implementation::ethereum_storage_model::caches::preimage::PreimageRequestForUnknownLength;
use crate::system_implementation::ethereum_storage_model::caches::EMPTY_STRING_KECCAK_HASH;
use crate::system_implementation::ethereum_storage_model::EMPTY_ROOT_HASH;
use core::alloc::Allocator;
use core::marker::PhantomData;
use evm_interpreter::errors::EvmSubsystemError;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::B160;
use ruint::aliases::U256;
use storage_models::common_structs::PreimageCacheModel;
use zk_ee::common_structs::cache_record::CacheRecord;
use zk_ee::common_structs::history_map::CacheSnapshotId;
use zk_ee::common_structs::history_map::HistoryMap;
use zk_ee::common_structs::history_map::HistoryMapItemRefMut;
use zk_ee::common_structs::structured_account_cache_record::*;
use zk_ee::common_structs::AccountCacheAppearance;
use zk_ee::common_structs::PreimageType;
use zk_ee::define_subsystem;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::interface_error;
use zk_ee::internal_error;
use zk_ee::memory::stack_trait::StackCtor;
use zk_ee::system::BalanceSubsystemError;
use zk_ee::system::Computational;
use zk_ee::system::DeconstructionSubsystemError;
use zk_ee::system::NonceError;
use zk_ee::system::NonceSubsystemError;
use zk_ee::system::Resource;
use zk_ee::system_io_oracle::SimpleOracleQuery;
use zk_ee::utils::BitsOrd;
use zk_ee::utils::Bytes32;
use zk_ee::wrap_error;
use zk_ee::{
    system::{
        errors::{internal::InternalError, system::SystemError},
        AccountData, AccountDataRequest, Ergs, Maybe, Resources,
    },
    system_io_oracle::IOOracle,
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
};

pub type AddressItem<'a, A> = HistoryMapItemRefMut<
    'a,
    BitsOrd<160, 3>,
    CacheRecord<EthereumAccountProperties, AccountPropertiesMetadataNoPubdata>,
    A,
    AccountCacheAppearance,
>;

pub struct EthereumAccountCache<
    A: Allocator + Clone, // = Global,
    R: Resources,
    SC: StackCtor<N>,
    const N: usize,
> {
    pub(crate) cache: HistoryMap<
        BitsOrd160,
        CacheRecord<EthereumAccountProperties, AccountPropertiesMetadataNoPubdata>,
        A,
        AccountCacheAppearance,
    >,
    pub(crate) current_tx_number: u32,
    #[allow(dead_code)]
    alloc: A,
    phantom: PhantomData<(R, SC)>,
}

impl<A: Allocator + Clone, R: Resources, SC: StackCtor<N>, const N: usize>
    EthereumAccountCache<A, R, SC, N>
{
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            cache: HistoryMap::new(allocator.clone()),
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
        address: &B160,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
        is_access_list: bool,
        observe: bool,
    ) -> Result<AddressItem<'_, A>, SystemError> {
        let ergs = match ee_type {
            ExecutionEnvironmentType::NoEE => {
                if is_access_list {
                    // For access lists, EVM charges the full cost as many
                    // times as an account is in the list.
                    ACCESS_LIST_TOUCH_ACCESS_COST_ERGS
                } else {
                    Ergs::empty()
                }
            }
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

        let mut initialized_element = false;

        self.cache
            .get_or_insert(address.into(), || {
                // Element doesn't exist in cache yet, initialize it
                initialized_element = true;

                // - first get a hash of properties from storage
                match ee_type {
                    ExecutionEnvironmentType::NoEE => {}
                    ExecutionEnvironmentType::EVM => {
                        let mut cost: R = if evm_interpreter::utils::is_precompile(&address) {
                            R::empty() // We've charged the access already.
                        } else {
                            R::from_ergs(COLD_PROPERTIES_ACCESS_EXTRA_COST_ERGS)
                        };
                        if is_selfdestruct {
                            // Selfdestruct doesn't charge for warm, but it
                            // includes the warm cost for cold access
                            cost.add_ergs(WARM_PROPERTIES_ACCESS_COST_ERGS)
                        };
                        resources.charge(&cost)?;
                    }
                }

                // we just ask the oracle for properties
                let acc_data = EthereumAccountPropertiesQuery::get(oracle, address)?;
                let initial_appearance = if acc_data.is_empty() {
                    AccountInitialAppearance::Unset
                } else {
                    AccountInitialAppearance::Retrieved
                };
                let current_appearance = if observe {
                    AccountCurrentAppearance::Observed
                } else {
                    AccountCurrentAppearance::Touched
                };
                let appearance =
                    AccountCacheAppearance::new(initial_appearance, current_appearance);

                // Note: we initialize it as cold, should be warmed up separately
                // Since in case of revert it should become cold again and initial record can't be rolled back
                Ok((CacheRecord::new(acc_data), appearance))
            })
            .and_then(|mut x| {
                // Warm up element according to EVM rules if needed
                let is_warm = x
                    .current()
                    .metadata()
                    .considered_warm(self.current_tx_number);
                if is_warm == false {
                    if initialized_element == false {
                        // Element exists in cache, but wasn't touched in current tx yet
                        match ee_type {
                            ExecutionEnvironmentType::NoEE => {}
                            ExecutionEnvironmentType::EVM => {
                                let mut cost: R = if evm_interpreter::utils::is_precompile(&address)
                                {
                                    R::empty() // We've charged the access already.
                                } else {
                                    R::from_ergs(COLD_PROPERTIES_ACCESS_EXTRA_COST_ERGS)
                                };
                                if is_selfdestruct {
                                    // Selfdestruct doesn't charge for warm, but it
                                    // includes the warm cost for cold access
                                    cost.add_ergs(WARM_PROPERTIES_ACCESS_COST_ERGS)
                                };
                                resources.charge(&cost)?;
                            }
                        }
                    }
                    // mark as warm
                    x.update(|cache_record| {
                        cache_record.update_metadata(|m| {
                            if is_warm == false {
                                assert!(m.is_marked_for_deconstruction == false); // any deconstuction should finish in previous TX
                                m.last_touched_in_tx = Some(self.current_tx_number);
                            }
                            Ok(())
                        })
                    })?;
                }
                // appearance mark
                if observe {
                    x.key_properties_mut().observe();
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
            false,
        )?;

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        let cur = account_data.current().value().balance;
        let new = update_fn(&cur)?;
        account_data.key_properties_mut().observe();
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
        from: &B160,
        to: &B160,
        amount: &U256,
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
        oracle: &mut impl IOOracle,
        is_access_list: bool,
        observe: bool,
    ) -> Result<(), SystemError> {
        self.materialize_element::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            oracle,
            false,
            is_access_list,
            observe,
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
        HasBytecode: Maybe<bool>,
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
                HasBytecode,
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
            HasBytecode,
        >,
        SystemError,
    > {
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            ee_type, resources, address, oracle, false, false, true,
        )?;
        // we are actually going to use account properties, so we should mark it so
        account_data.key_properties_mut().observe();
        let initial_appearance = account_data.key_properties().initial_appearance();
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
        let bytecode = if needs_preimage {
            // NOTE: deconstruction happens at the end of the TX, so even deconstructed accounts would NOT
            // respond with empty bytecode (well, WTF)

            {
                if bytecode_hash_is_zero {
                    debug_assert!(initial_appearance == AccountInitialAppearance::Unset);

                    let res: &'static [u8] = &[];

                    res
                } else if full_data.bytecode_hash == EMPTY_STRING_KECCAK_HASH {
                    let res: &'static [u8] = &[];

                    res
                } else {
                    // can try to get preimage
                    let preimage_type = PreimageRequestForUnknownLength {
                        hash: full_data.bytecode_hash,
                        preimage_type: PreimageType::Bytecode,
                    };
                    preimages_cache.get_preimage::<PROOF_ENV>(
                        ee_type,
                        &preimage_type,
                        resources,
                        oracle,
                    )?
                }
            }
        } else {
            &[]
        };

        let code_length = bytecode.len() as u32;

        let is_delegated = if code_length == 3 + 20 {
            bytecode[..3] == zk_ee::system::EIP7702_DELEGATION_MARKER
        } else {
            false
        };

        let has_bytecode =
            !(bytecode_hash_is_zero || full_data.bytecode_hash == EMPTY_STRING_KECCAK_HASH);

        Ok(AccountData {
            ee_version: Maybe::construct(|| ExecutionEnvironmentType::EVM as u8),
            observable_bytecode_hash: Maybe::construct(|| full_data.bytecode_hash),
            observable_bytecode_len: Maybe::construct(|| code_length),
            nonce: Maybe::construct(|| full_data.nonce),
            bytecode_hash: Maybe::construct(|| full_data.bytecode_hash),
            unpadded_code_len: Maybe::construct(|| code_length),
            artifacts_len: Maybe::construct(|| 0),
            nominal_token_balance: Maybe::construct(|| full_data.balance),
            bytecode: Maybe::construct(|| bytecode),
            code_version: Maybe::construct(|| 0),
            is_delegated: Maybe::construct(|| is_delegated),
            has_bytecode: Maybe::construct(|| has_bytecode),
        })
    }

    pub fn increment_nonce<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        increment_by: u64,
        oracle: &mut impl IOOracle,
    ) -> Result<u64, NonceSubsystemError> {
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            ee_type, resources, address, oracle, false, false, false,
        )?;

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        let nonce = account_data.current().value().nonce;
        if let Some(new_nonce) = nonce.checked_add(increment_by) {
            account_data.key_properties_mut().observe();
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
        address: &B160,
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
        from: &B160,
        to: &B160,
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

                Ok(Bytes32::from_array(Keccak256::digest(observable_bytecode)))
            }
        }
    }

    pub fn deploy_code<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: &B160,
        deployed_code: &[u8],
        preimages_cache: &mut BytecodeKeccakPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError> {
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

        account_data.key_properties_mut().observe();
        account_data.update(|cache_record| {
            cache_record.update(|v, m| {
                v.bytecode_hash = bytecode_hash;

                m.deployed_in_tx = Some(cur_tx);

                Ok(())
            })
        })?;

        Ok(deployed_code)
    }

    pub fn mark_for_deconstruction<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: &B160,
        nominal_token_beneficiary: &B160,
        oracle: &mut impl IOOracle,
        in_constructor: bool,
    ) -> Result<(), DeconstructionSubsystemError> {
        let cur_tx = self.current_tx_number;
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            from_ee, resources, at_address, oracle, true, false, false,
        )?;
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
        let should_be_deconstructed =
            account_data.current().metadata().deployed_in_tx == Some(cur_tx) || in_constructor;

        if should_be_deconstructed {
            account_data.key_properties_mut().observe();
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
                    let entry = match self.cache.get(nominal_token_beneficiary.into()) {
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
                        let ergs_to_spend = Ergs(NEWACCOUNT * ERGS_PER_GAS);
                        resources.charge(&R::from_ergs(ergs_to_spend))?;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn set_delegation<const PROOF_ENV: bool>(
        &mut self,
        resources: &mut R,
        at_address: &B160,
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

        account_data.key_properties_mut().observe();
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
        storage: &mut EthereumStorageCache<A, SC, N, R, P>,
    ) -> Result<(), InternalError> {
        // Actually deconstructing accounts
        self.cache.apply_to_last_record_of_pending_changes(
            |key, (initial, current), cache_appearance| {
                if current.value.metadata().is_marked_for_deconstruction {
                    // NOTE: initially account had 0 nonce, but it could be "material",
                    // with state root being empty, and bytecode hash being hash of empty string.

                    // NOTE: Balance will be zeroed out if deconstruction happens here
                    let initially_empty =
                        cache_appearance.initial_appearance() == AccountInitialAppearance::Unset;
                    cache_appearance.assert_observed();
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
                        .slot_values
                        .clear_state_impl(key)
                        .expect("must clear state for code deconstruction in same TX");
                }
                Ok(())
            },
        )?;

        Ok(())
    }

    ///
    /// Returns slots that were changed during execution.
    ///
    pub fn net_diffs_iter(
        &self,
    ) -> impl Iterator<Item = (B160, (u64, U256, Bytes32))> + use<'_, A, SC, N, R> {
        self.cache
            .iter()
            .filter(|v| v.initial().value() != v.current().value())
            .map(|v| {
                let address = v.key().0;
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
