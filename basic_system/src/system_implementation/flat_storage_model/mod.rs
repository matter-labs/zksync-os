//!
//! This module contains flat(aka new ZKsyncOS) storage model implementation.
//!
//! It's fixed height merkle tree with linked list in the leaves sorted by storage keys.
//! Account data hashes stored in this tree and published separately.
//!
pub mod account_cache;
mod account_cache_entry;
pub mod cost_constants;
pub mod preimage_cache;
mod simple_growable_storage;
pub mod storage_cache;

pub use self::account_cache::*;
pub use self::account_cache_entry::*;
pub use self::preimage_cache::*;
pub use self::simple_growable_storage::*;
pub use self::storage_cache::*;
use core::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::B160;
use storage_models::common_structs::snapshottable_io::SnapshottableIo;
use storage_models::common_structs::StorageCacheModel;
use storage_models::common_structs::StorageModel;
use zk_ee::common_structs::{derive_flat_storage_key_with_hasher, ValueDiffCompressionStrategy};
use zk_ee::internal_error;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::BalanceSubsystemError;
use zk_ee::system::DeconstructionSubsystemError;
use zk_ee::system::NonceSubsystemError;
use zk_ee::system::Resources;
use zk_ee::{
    common_structs::{
        history_map::CacheSnapshotId, state_root_view::StateRootView, WarmStorageKey,
    },
    execution_environment_type::ExecutionEnvironmentType,
    memory::stack_trait::{StackCtor, StackCtorConst},
    system::{
        errors::system::SystemError, logger::Logger, AccountData, AccountDataRequest,
        IOResultKeeper, Maybe,
    },
    system_io_oracle::IOOracle,
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::Bytes32,
};

use super::system::ExtraCheck;

pub fn address_into_special_storage_key(address: &B160) -> Bytes32 {
    let mut key = Bytes32::zero();
    key.as_u8_array_mut()[12..].copy_from_slice(&address.to_be_bytes::<{ B160::BYTES }>());

    key
}

pub const TREE_HEIGHT: usize = 64;

// This model only touches storage related things, even though preimages cache can be reused
// by "signals" in theory, but we do not expect that in practice

pub struct FlatTreeWithAccountsUnderHashesStorageModel<
    A: Allocator + Clone,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
    SC: StackCtor<SCC>,
    SCC: const StackCtorConst,
    const PROOF_ENV: bool,
> where
    ExtraCheck<SCC, A>:,
{
    pub(crate) storage_cache: NewStorageWithAccountPropertiesUnderHash<A, SC, SCC, R, P>,
    pub(crate) preimages_cache: BytecodeAndAccountDataPreimagesStorage<R, A>,
    pub(crate) account_data_cache: NewModelAccountCache<A, R, P, SC, SCC>,
    pub(crate) allocator: A,
}

pub struct FlatTreeWithAccountsUnderHashesStorageModelStateSnapshot {
    storage: CacheSnapshotId,
    account_data: CacheSnapshotId,
    preimages: CacheSnapshotId,
}

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        const PROOF_ENV: bool,
    > StorageModel for FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SC, SCC, PROOF_ENV>
where
    ExtraCheck<SCC, A>:,
{
    type Allocator = A;
    type Resources = R;
    type StorageCommitment = FlatStorageCommitment<TREE_HEIGHT>;

    type IOTypes = EthereumIOTypesConfig;
    type InitData = P;

    fn finish_tx(&mut self) -> Result<(), zk_ee::system::errors::internal::InternalError> {
        self.account_data_cache.finish_tx(&mut self.storage_cache)
    }

    fn construct(init_data: Self::InitData, allocator: Self::Allocator) -> Self {
        let resources_policy = init_data;
        let storage_cache = NewStorageWithAccountPropertiesUnderHash::<A, SC, SCC, R, P>(
            GenericPubdataAwarePlainStorage::new_from_parts(allocator.clone(), resources_policy),
        );
        let preimages_cache =
            BytecodeAndAccountDataPreimagesStorage::<R, A>::new_from_parts(allocator.clone());
        let account_data_cache =
            NewModelAccountCache::<A, R, P, SC, SCC>::new_from_parts(allocator.clone());

        Self {
            storage_cache,
            preimages_cache,
            account_data_cache,
            allocator,
        }
    }

    fn pubdata_used_by_tx(&self) -> u32 {
        self.account_data_cache.calculate_pubdata_used_by_tx()
            + self.storage_cache.calculate_pubdata_used_by_tx()
    }

    fn finish(
        self,
        oracle: &mut impl IOOracle,
        state_commitment: Option<&mut Self::StorageCommitment>,
        pubdata_hasher: &mut impl MiniDigest,
        result_keeper: &mut impl IOResultKeeper<Self::IOTypes>,
        logger: &mut impl Logger,
    ) -> Result<(), InternalError> {
        let Self {
            mut storage_cache,
            mut preimages_cache,
            mut account_data_cache,
            allocator,
        } = self;
        // flush accounts into storage
        account_data_cache
            .persist_changes(
                &mut storage_cache,
                &mut preimages_cache,
                oracle,
                result_keeper,
            )
            .expect("must persist changes from account cache");

        // 1. Return uncompressed state diffs for sequencer
        result_keeper.storage_diffs(storage_cache.net_diffs_iter().map(|(k, v)| {
            let WarmStorageKey { address, key } = k;
            let value = v.current_value;
            (address, key, value)
        }));
        preimages_cache.report_new_preimages(result_keeper)?;

        // 2. Commit to/return compressed pubdata
        let encdoded_state_diffs_count =
            (storage_cache.net_diffs_iter().count() as u32).to_be_bytes();
        pubdata_hasher.update(&encdoded_state_diffs_count);
        result_keeper.pubdata(&encdoded_state_diffs_count);

        let mut hasher = crypto::blake2s::Blake2s256::new();
        storage_cache
            .0
            .cache
            .apply_to_all_updated_elements::<_, ()>(|l, r, k| {
                // TODO(EVM-1074): use tree index instead of key for repeated writes
                let derived_key =
                    derive_flat_storage_key_with_hasher(&k.address, &k.key, &mut hasher);
                pubdata_hasher.update(derived_key.as_u8_ref());
                result_keeper.pubdata(derived_key.as_u8_ref());

                if l.value() == r.value() {
                    return Ok(());
                }
                // we publish preimages for account details
                if k.address == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
                    let account_address = B160::try_from_be_slice(&k.key.as_u8_ref()[12..])
                        .unwrap()
                        .into();
                    let cache_item = account_data_cache.cache.get(&account_address).ok_or(())?;
                    let (l, r) = cache_item.get_initial_and_last_values().ok_or(())?;
                    AccountProperties::diff_compression::<PROOF_ENV, _, _>(
                        l.value(),
                        r.value(),
                        r.metadata().not_publish_bytecode,
                        pubdata_hasher,
                        result_keeper,
                        &mut preimages_cache,
                        oracle,
                    )
                    .map_err(|_| ())?;
                } else {
                    ValueDiffCompressionStrategy::optimal_compression(
                        l.value(),
                        r.value(),
                        pubdata_hasher,
                        result_keeper,
                    );
                }
                Ok(())
            })
            .map_err(|_| internal_error!("Failed to compute pubdata"))?;

        // 3. Verify/apply reads and writes
        cycle_marker::wrap!("verify_and_apply_batch", {
            if let Some(state_commitment) = state_commitment {
                let it = storage_cache.net_accesses_iter();
                state_commitment.verify_and_apply_batch(oracle, it, allocator, logger)
            } else {
                Ok(())
            }
        })?;
        Ok(())
    }

    fn storage_read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError> {
        self.storage_cache
            .read(ee_type, resources, address, key, oracle)
    }

    fn storage_touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError> {
        self.storage_cache
            .touch(ee_type, resources, address, key, oracle, is_access_list)
    }

    fn storage_write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        new_value: &<Self::IOTypes as SystemIOTypesConfig>::StorageValue,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError> {
        self.storage_cache
            .write(ee_type, resources, address, key, new_value, oracle)
    }

    fn read_account_properties<
        EEVersion: Maybe<u8>,
        ObservableBytecodeHash: Maybe<<Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue>,
        ObservableBytecodeLen: Maybe<u32>,
        Nonce: Maybe<u64>,
        BytecodeHash: Maybe<<Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue>,
        BytecodeLen: Maybe<u32>,
        ArtifactsLen: Maybe<u32>,
        NominalTokenBalance: Maybe<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue>,
        Bytecode: Maybe<&'static [u8]>,
        CodeVersion: Maybe<u8>,
    >(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        request: AccountDataRequest<
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
            >,
        >,
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
        >,
        SystemError,
    > {
        self.account_data_cache
            .read_account_properties::<PROOF_ENV, _, _, _, _, _, _, _, _, _, _>(
                ee_type,
                resources,
                address,
                request,
                &mut self.storage_cache,
                &mut self.preimages_cache,
                oracle,
            )
    }

    fn touch_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError> {
        self.account_data_cache.touch_account::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            &mut self.storage_cache,
            &mut self.preimages_cache,
            oracle,
            is_access_list,
        )
    }

    fn get_selfbalance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue, SystemError> {
        self.account_data_cache
            .read_account_balance_assuming_warm(ee_type, resources, address)
    }

    fn deploy_code(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        bytecode: &[u8],
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError> {
        self.account_data_cache.deploy_code::<PROOF_ENV>(
            from_ee,
            resources,
            at_address,
            bytecode,
            &mut self.storage_cache,
            &mut self.preimages_cache,
            oracle,
        )
    }

    fn set_bytecode_details(
        &mut self,
        resources: &mut R,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        ee: ExecutionEnvironmentType,
        bytecode_hash: Bytes32,
        bytecode_len: u32,
        artifacts_len: u32,
        observable_bytecode_hash: Bytes32,
        observable_bytecode_len: u32,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        self.account_data_cache.set_bytecode_details::<PROOF_ENV>(
            resources,
            at_address,
            ee,
            bytecode_hash,
            bytecode_len,
            artifacts_len,
            observable_bytecode_hash,
            observable_bytecode_len,
            &mut self.storage_cache,
            &mut self.preimages_cache,
            oracle,
        )
    }

    fn mark_for_deconstruction(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        nominal_token_beneficiary: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
        in_constructor: bool,
    ) -> Result<(), DeconstructionSubsystemError> {
        self.account_data_cache
            .mark_for_deconstruction::<PROOF_ENV>(
                from_ee,
                resources,
                at_address,
                nominal_token_beneficiary,
                &mut self.storage_cache,
                &mut self.preimages_cache,
                oracle,
                in_constructor,
            )
    }

    fn increment_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        increment_by: u64,
        oracle: &mut impl IOOracle,
    ) -> Result<u64, NonceSubsystemError> {
        self.account_data_cache.increment_nonce::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            increment_by,
            &mut self.storage_cache,
            &mut self.preimages_cache,
            oracle,
        )
    }

    fn transfer_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        from: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        to: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        amount: &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        oracle: &mut impl IOOracle,
    ) -> Result<(), BalanceSubsystemError> {
        self.account_data_cache
            .transfer_nominal_token_value::<PROOF_ENV>(
                from_ee,
                resources,
                from,
                to,
                amount,
                &mut self.storage_cache,
                &mut self.preimages_cache,
                oracle,
            )
    }

    fn update_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        update_fn: impl FnOnce(
            &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        ) -> Result<
            <Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
            BalanceSubsystemError,
        >,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue, BalanceSubsystemError>
    {
        self.account_data_cache
            .update_nominal_token_value::<PROOF_ENV>(
                from_ee,
                resources,
                address,
                update_fn,
                &mut self.storage_cache,
                &mut self.preimages_cache,
                oracle,
            )
    }
}

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        const PROOF_ENV: bool,
    > SnapshottableIo for FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SC, SCC, PROOF_ENV>
where
    ExtraCheck<SCC, A>:,
{
    type StateSnapshot = FlatTreeWithAccountsUnderHashesStorageModelStateSnapshot;

    fn begin_new_tx(&mut self) {
        self.storage_cache.begin_new_tx();
        self.preimages_cache.begin_new_tx();
        self.account_data_cache.begin_new_tx();
    }

    fn start_frame(&mut self) -> Self::StateSnapshot {
        let storage_handle = self.storage_cache.start_frame();
        let preimages_handle = self.preimages_cache.start_frame();
        let account_handle = self.account_data_cache.start_frame();

        FlatTreeWithAccountsUnderHashesStorageModelStateSnapshot {
            storage: storage_handle,
            preimages: preimages_handle,
            account_data: account_handle,
        }
    }

    fn finish_frame(
        &mut self,
        rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError> {
        self.storage_cache
            .finish_frame(rollback_handle.map(|x| &x.storage))?;
        self.preimages_cache
            .finish_frame(rollback_handle.map(|x| &x.preimages))?;
        self.account_data_cache
            .finish_frame(rollback_handle.map(|x| &x.account_data))?;

        Ok(())
    }
}
