//! Account cache, backed by a history map.
//! This caches the actual account data, which will
//! then be published into the preimage storage.
use super::AccountPropertiesMetadata;
use super::BytecodeAndAccountDataPreimagesStorage;
use super::NewStorageWithAccountPropertiesUnderHash;
use crate::system_implementation::flat_storage_model::account_cache_entry::AccountProperties;
use crate::system_implementation::flat_storage_model::cost_constants::*;
use crate::system_implementation::flat_storage_model::PreimageRequest;
use crate::system_implementation::flat_storage_model::StorageAccessPolicy;
use crate::system_implementation::flat_storage_model::DEFAULT_CODE_VERSION_BYTE;
use crate::system_implementation::system::ExtraCheck;
use alloc::collections::BTreeSet;
use core::alloc::Allocator;
use core::marker::PhantomData;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::B160;
use ruint::aliases::U256;
use storage_models::common_structs::AccountAggregateDataHash;
use storage_models::common_structs::PreimageCacheModel;
use storage_models::common_structs::StorageCacheModel;
use zk_ee::common_structs::cache_record::Appearance;
use zk_ee::common_structs::cache_record::CacheRecord;
use zk_ee::common_structs::history_map::CacheSnapshotId;
use zk_ee::common_structs::history_map::HistoryMap;
use zk_ee::common_structs::history_map::HistoryMapItemRefMut;
use zk_ee::common_structs::PreimageType;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::memory::stack_trait::StackCtor;
use zk_ee::system::Computational;
use zk_ee::system::Resource;
use zk_ee::utils::BitsOrd;
use zk_ee::utils::Bytes32;
use zk_ee::{
    memory::stack_trait::StackCtorConst,
    system::{
        errors::{InternalError, SystemError, UpdateQueryError},
        AccountData, AccountDataRequest, Ergs, IOResultKeeper, Maybe, Resources,
    },
    system_io_oracle::IOOracle,
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
};

pub type BitsOrd160 = BitsOrd<{ B160::BITS }, { B160::LIMBS }>;
type AddressItem<'a, A> = HistoryMapItemRefMut<
    'a,
    BitsOrd<160, 3>,
    CacheRecord<AccountProperties, AccountPropertiesMetadata>,
    A,
>;

pub struct NewModelAccountCache<
    A: Allocator + Clone, // = Global,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
    SC: StackCtor<SCC>,
    SCC: const StackCtorConst,
> where
    ExtraCheck<SCC, A>:,
{
    pub(crate) cache:
        HistoryMap<BitsOrd160, CacheRecord<AccountProperties, AccountPropertiesMetadata>, A>,
    pub(crate) current_tx_number: u32,
    alloc: A,
    phantom: PhantomData<(R, P, SC, SCC)>,
}

impl<
        A: Allocator + Clone,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
    > NewModelAccountCache<A, R, P, SC, SCC>
where
    ExtraCheck<SCC, A>:,
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
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut impl PreimageCacheModel<Resources = R, PreimageRequest = PreimageRequest>,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
        is_access_list: bool,
    ) -> Result<AddressItem<A>, SystemError> {
        let ergs = match ee_type {
            ExecutionEnvironmentType::NoEE => {
                if is_access_list {
                    // For access lists, EVM charges the full cost as many
                    // times as an account is in the list.
                    Ergs(2400 * ERGS_PER_GAS)
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
            _ => return Err(internal_error!("Unsupported EE").into()),
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
                    _ => return Err(internal_error!("Unsupported EE").into()),
                }

                // to avoid divergence we read as-if infinite ergs
                let hash = resources.with_infinite_ergs(|inf_resources| {
                    storage.read_special_account_property::<AccountAggregateDataHash>(
                        ExecutionEnvironmentType::NoEE,
                        inf_resources,
                        address,
                        oracle,
                    )
                })?;

                let acc_data = match hash == Bytes32::ZERO {
                    true => (AccountProperties::default(), Appearance::Unset),
                    false => {
                        let preimage = preimages_cache.get_preimage::<PROOF_ENV>(
                            ee_type,
                            &PreimageRequest {
                                hash,
                                expected_preimage_len_in_bytes: AccountProperties::ENCODED_SIZE
                                    as u32,
                                preimage_type: PreimageType::AccountData,
                            },
                            resources,
                            oracle,
                        )?;
                        // it's redundant as preimages cache should just check it, but why not
                        assert_eq!(preimage.len(), AccountProperties::ENCODED_SIZE);

                        let props =
                            AccountProperties::decode(preimage.try_into().map_err(|_| {
                                internal_error!("Unexpected preimage length for AccountProperties")
                            })?);

                        (props, Appearance::Retrieved)
                    }
                };

                // Note: we initialize it as cold, should be warmed up separately
                // Since in case of revert it should become cold again and initial record can't be rolled back
                Ok(CacheRecord::new(acc_data.0, acc_data.1))
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
                            _ => return Err(internal_error!("Unsupported EE").into()),
                        }
                    }

                    x.update(|cache_record| {
                        cache_record.update_metadata(|m| {
                            m.last_touched_in_tx = Some(self.current_tx_number);
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
        update_fn: impl FnOnce(&U256) -> Result<U256, UpdateQueryError>,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut impl PreimageCacheModel<Resources = R, PreimageRequest = PreimageRequest>,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
    ) -> Result<U256, UpdateQueryError> {
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            storage,
            preimages_cache,
            oracle,
            is_selfdestruct,
            false,
        )?;

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        let cur = account_data.current().value().balance;
        let new = update_fn(&cur)?;
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
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut impl PreimageCacheModel<Resources = R, PreimageRequest = PreimageRequest>,
        oracle: &mut impl IOOracle,
        is_selfdestruct: bool,
    ) -> Result<(), UpdateQueryError> {
        let mut f = |addr, op: fn(U256, U256) -> (U256, bool)| {
            self.update_nominal_token_value_inner::<PROOF_ENV>(
                from_ee,
                resources,
                addr,
                move |old_balance: &U256| {
                    let (new_value, of) = op(*old_balance, *amount);
                    if of {
                        Err(UpdateQueryError::NumericBoundsError)
                    } else {
                        Ok(new_value)
                    }
                },
                storage,
                preimages_cache,
                oracle,
                is_selfdestruct,
            )
        };

        // can do update twice
        f(from, U256::overflowing_sub)?;
        f(to, U256::overflowing_add)?;

        Ok(())
    }

    // special method, not part of the trait as it's not overly generic
    pub fn persist_changes(
        &self,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
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

            let _ = preimages_cache.record_preimage::<false>(
                ExecutionEnvironmentType::NoEE,
                &(PreimageRequest {
                    hash: properties_hash,
                    expected_preimage_len_in_bytes: AccountProperties::ENCODED_SIZE as u32,
                    preimage_type: PreimageType::AccountData,
                }),
                &mut inf_resources,
                &encoding,
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

            pubdata_used += AccountProperties::diff_compression_length(
                initial.value(),
                current.value(),
                current.metadata().not_publish_bytecode,
            )
            .unwrap();
        }

        pubdata_used
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
            _ => return Err(internal_error!("Unsupported EE").into()),
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
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError> {
        self.materialize_element::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            storage,
            preimages_cache,
            oracle,
            false,
            is_access_list,
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
            >,
        >,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
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
            false,
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
            bytecode_len: Maybe::construct(|| full_data.bytecode_len),
            artifacts_len: Maybe::construct(|| full_data.artifacts_len),
            nominal_token_balance: Maybe::construct(|| full_data.balance),
            bytecode: Maybe::try_construct(|| {
                // we charged for "cold" behavior already, so we just ask for preimage

                if full_data.bytecode_hash.is_zero() {
                    assert!(full_data.observable_bytecode_hash.is_zero());
                    assert_eq!(full_data.bytecode_len, 0);
                    assert_eq!(full_data.artifacts_len, 0);
                    assert_eq!(full_data.observable_bytecode_len, 0);

                    let res: &'static [u8] = &[];
                    Ok(res)
                } else {
                    // can try to get preimage
                    // TODO(EVM-1073): compute preimage len using artifacts and bytecode len, and EE type in our model
                    let preimage_type = PreimageRequest {
                        hash: full_data.bytecode_hash,
                        expected_preimage_len_in_bytes: full_data.bytecode_len,
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
        })
    }

    pub fn increment_nonce<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        increment_by: u64,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<u64, UpdateQueryError> {
        let mut account_data = self.materialize_element::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            storage,
            preimages_cache,
            oracle,
            false,
            false,
        )?;

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
            return Err(UpdateQueryError::NumericBoundsError);
        }

        Ok(nonce)
    }

    pub fn update_nominal_token_value<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        update_fn: impl FnOnce(&U256) -> Result<U256, UpdateQueryError>,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<U256, UpdateQueryError> {
        self.update_nominal_token_value_inner::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            update_fn,
            storage,
            preimages_cache,
            oracle,
            false,
        )
    }

    pub fn transfer_nominal_token_value<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        from: &B160,
        to: &B160,
        amount: &U256,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(), UpdateQueryError> {
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

    pub fn deploy_code<const PROOF_ENV: bool>(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: &B160,
        bytecode: &[u8],
        bytecode_len: u32,
        artifacts_len: u32,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError> {
        // Charge for code deposit cost
        match from_ee {
            ExecutionEnvironmentType::NoEE => (),
            ExecutionEnvironmentType::EVM => {
                use evm_interpreter::gas_constants::CODEDEPOSIT;
                let code_deposit_cost = CODEDEPOSIT.saturating_mul(bytecode_len.into());
                let ergs_to_spend = Ergs(code_deposit_cost.saturating_mul(ERGS_PER_GAS));
                resources.charge(&R::from_ergs(ergs_to_spend))?;
            }
            _ => todo!(),
        }

        // we charged for everything, and so all IO below will use infinite ergs
        // We've checked that this account is empty in `prepare_for_deployment`.

        let cur_tx = self.current_tx_number;

        let mut account_data = resources.with_infinite_ergs(|inf_resources| {
            self.materialize_element::<PROOF_ENV>(
                from_ee,
                inf_resources,
                at_address,
                storage,
                preimages_cache,
                oracle,
                false,
                false,
            )
        })?;

        // compute observable and true hashes of bytecode
        let observable_bytecode_hash = match from_ee {
            ExecutionEnvironmentType::EVM => {
                assert_eq!(artifacts_len, 0);
                use crypto::sha3::Keccak256;
                use crypto::MiniDigest;
                let digest = Keccak256::digest(bytecode);
                Bytes32::from_array(digest)
            }
            _ => {
                return Err(internal_error!("Unsupported EE").into());
            }
        };

        let bytecode_hash = match from_ee {
            ExecutionEnvironmentType::EVM => {
                assert_eq!(artifacts_len, 0);
                use crypto::blake2s::Blake2s256;
                use crypto::MiniDigest;
                let digest = Blake2s256::digest(bytecode);
                Bytes32::from_array(digest)
            }
            _ => {
                return Err(internal_error!("Unsupported EE").into());
            }
        };

        // save bytecode

        // TODO(EVM-1073): compute preimage len using bytecode and artifacts len, and EE type
        let bytecode = preimages_cache.record_preimage::<PROOF_ENV>(
            from_ee,
            &(PreimageRequest {
                hash: bytecode_hash,
                expected_preimage_len_in_bytes: bytecode_len,
                preimage_type: PreimageType::Bytecode,
            }),
            resources,
            bytecode,
        )?;

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        account_data.update(|cache_record| {
            cache_record.update(|v, m| {
                v.observable_bytecode_hash = observable_bytecode_hash;
                v.observable_bytecode_len = bytecode_len;
                v.bytecode_hash = bytecode_hash;
                v.bytecode_len = bytecode_len;
                v.artifacts_len = artifacts_len;
                v.versioning_data.set_as_deployed();
                v.versioning_data.set_ee_version(from_ee as u8);
                v.versioning_data
                    .set_code_version(DEFAULT_CODE_VERSION_BYTE);

                m.deployed_in_tx = Some(cur_tx);
                // This is unlikely to happen, this case shouldn't be reachable by higher level logic
                // but just in case if force deployed contract was redeployed with regular deployment we want to publish it
                m.not_publish_bytecode = false;

                Ok(())
            })
        })?;

        Ok(bytecode)
    }

    pub fn set_bytecode_details<const PROOF_ENV: bool>(
        &mut self,
        resources: &mut R,
        at_address: &B160,
        ee: ExecutionEnvironmentType,
        bytecode_hash: Bytes32,
        bytecode_len: u32,
        artifacts_len: u32,
        observable_bytecode_hash: Bytes32,
        observable_bytecode_len: u32,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        let cur_tx = self.current_tx_number;

        let mut account_data = resources.with_infinite_ergs(|inf_resources| {
            self.materialize_element::<PROOF_ENV>(
                ExecutionEnvironmentType::NoEE,
                inf_resources,
                at_address,
                storage,
                preimages_cache,
                oracle,
                false,
                false,
            )
        })?;

        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;

        account_data.update(|cache_record| {
            cache_record.update(|v, m| {
                v.observable_bytecode_hash = observable_bytecode_hash;
                v.observable_bytecode_len = observable_bytecode_len;
                v.bytecode_hash = bytecode_hash;
                v.bytecode_len = bytecode_len;
                v.artifacts_len = artifacts_len;
                v.versioning_data.set_as_deployed();
                v.versioning_data.set_ee_version(ee as u8);
                v.versioning_data
                    .set_code_version(DEFAULT_CODE_VERSION_BYTE);

                m.deployed_in_tx = Some(cur_tx);
                m.not_publish_bytecode = true;

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
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
        in_constructor: bool,
    ) -> Result<(), SystemError> {
        let cur_tx = self.current_tx_number;
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
            account_data.update::<_, SystemError>(|cache_record| {
                cache_record.deconstruct();
                Ok(())
            })?
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
            .map_err(|e| match e {
                UpdateQueryError::NumericBoundsError => {
                    internal_error!("Impossible, not enough balance in deconstruction").into()
                }
                UpdateQueryError::System(e) => e,
            })?
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
                        && beneficiary_properties.bytecode_len == 0
                        // We need to check with the transferred amount,
                        // this means it was 0 before the transfer.
                        && beneficiary_properties.balance == transfer_amount;
                    if beneficiary_is_empty {
                        use evm_interpreter::gas_constants::NEWACCOUNT;
                        let ergs_to_spend = Ergs(NEWACCOUNT * ERGS_PER_GAS);
                        resources.charge(&R::from_ergs(ergs_to_spend))?;
                    }
                }
                _ => return Err(internal_error!("Unsupported EE").into()),
            }
        }

        Ok(())
    }

    pub fn finish_tx(
        &mut self,
        storage: &mut NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
    ) -> Result<(), InternalError> {
        // Actually deconstructing accounts
        self.cache
            .apply_to_last_record_of_pending_changes(|key, head_history_record| {
                if head_history_record.value.appearance() == Appearance::Deconstructed {
                    head_history_record.value.update(|x, _| {
                        *x = AccountProperties::TRIVIAL_VALUE;
                        Ok(())
                    })?;
                    storage
                        .0
                        .clear_state_impl(key)
                        .expect("must clear state for code deconstruction in same TX");
                }
                Ok(())
            })?;

        Ok(())
    }
}
