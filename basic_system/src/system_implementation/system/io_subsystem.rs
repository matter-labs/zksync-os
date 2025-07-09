//! Implementation of the IO subsystem.
use super::*;
use crate::system_functions::keccak256::keccak256_native_cost;
use crate::system_functions::keccak256::Keccak256Impl;
use cost_constants::EVENT_DATA_PER_BYTE_COST;
use cost_constants::EVENT_STORAGE_BASE_NATIVE_COST;
use cost_constants::EVENT_TOPIC_NATIVE_COST;
use cost_constants::WARM_TSTORAGE_READ_NATIVE_COST;
use cost_constants::WARM_TSTORAGE_WRITE_NATIVE_COST;
use crypto::blake2s::Blake2s256;
use crypto::MiniDigest;
use errors::SystemFunctionError;
use evm_interpreter::gas_constants::LOG;
use evm_interpreter::gas_constants::LOGDATA;
use evm_interpreter::gas_constants::LOGTOPIC;
use evm_interpreter::gas_constants::TLOAD;
use evm_interpreter::gas_constants::TSTORE;
use storage_models::common_structs::generic_transient_storage::GenericTransientStorage;
use storage_models::common_structs::snapshottable_io::SnapshottableIo;
use storage_models::common_structs::StorageModel;
use zk_ee::common_structs::BasicIOImplementerFSM;
use zk_ee::common_structs::L2_TO_L1_LOG_SERIALIZE_SIZE;
use zk_ee::internal_error;
use zk_ee::out_of_ergs_error;
use zk_ee::system::metadata::BlockMetadataFromOracle;
use zk_ee::{
    common_structs::{EventsStorage, LogsStorage},
    kv_markers::UsizeDeserializable,
    memory::ArrayBuilder,
    system::{
        errors::{SystemError, UpdateQueryError},
        AccountData, AccountDataRequest, EthereumLikeIOSubsystem, IOResultKeeper, IOSubsystem,
        IOSubsystemExt, Maybe,
    },
    system_io_oracle::InitializeIOImplementerIterator,
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::UsizeAlignedByteBox,
};

pub struct FullIO<
    A: Allocator + Clone + Default,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
    SC: StackCtor<SCC>,
    SCC: const StackCtorConst,
    O: IOOracle,
    const PROOF_ENV: bool,
> where
    ExtraCheck<SCC, A>:,
{
    pub(crate) storage: FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SC, SCC, PROOF_ENV>,
    pub(crate) transient_storage: GenericTransientStorage<WarmStorageKey, Bytes32, SC, SCC, A>,
    pub(crate) logs_storage: LogsStorage<SC, SCC, A>,
    pub(crate) events_storage: EventsStorage<MAX_EVENT_TOPICS, SC, SCC, A>,
    pub(crate) allocator: A,
    pub(crate) oracle: O,
    pub(crate) tx_number: u32,
}

pub struct FullIOStateSnapshot {
    io: FlatTreeWithAccountsUnderHashesStorageModelStateSnapshot,
    transient: CacheSnapshotId,
    messages: usize,
    events: usize,
}

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        O: IOOracle,
        const PROOF_ENV: bool,
    > IOSubsystem for FullIO<A, R, P, SC, SCC, O, PROOF_ENV>
where
    ExtraCheck<SCC, A>:,
{
    type IOTypes = EthereumIOTypesConfig;
    type Resources = R;
    type StateSnapshot = FullIOStateSnapshot;

    fn storage_read<const TRANSIENT: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageValue, SystemError> {
        if TRANSIENT {
            let ergs = match ee_type {
                ExecutionEnvironmentType::EVM => Ergs(TLOAD * ERGS_PER_GAS),
                _ => return Err(internal_error!("Unsupported EE").into()),
            };
            let native = R::Native::from_computational(WARM_TSTORAGE_READ_NATIVE_COST);
            resources.charge(&R::from_ergs_and_native(ergs, native))?;

            let key = WarmStorageKey {
                address: *address,
                key: *key,
            };

            let mut result = Bytes32::ZERO;
            self.transient_storage.apply_read(&key, &mut result)?;

            Ok(result)
        } else {
            self.storage
                .storage_read(ee_type, resources, address, key, &mut self.oracle)
        }
    }

    fn storage_write<const TRANSIENT: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        value_to_write: &<Self::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) -> Result<(), SystemError> {
        if TRANSIENT {
            let ergs = match ee_type {
                ExecutionEnvironmentType::EVM => Ergs(TSTORE * ERGS_PER_GAS),
                _ => return Err(internal_error!("Unsupported EE").into()),
            };
            let native = R::Native::from_computational(WARM_TSTORAGE_WRITE_NATIVE_COST);
            resources.charge(&R::from_ergs_and_native(ergs, native))?;

            let key = WarmStorageKey {
                address: *address,
                key: *key,
            };
            self.transient_storage.apply_write(&key, value_to_write)?;

            Ok(())
        } else {
            let _ = self.storage.storage_write(
                ee_type,
                resources,
                address,
                key,
                value_to_write,
                &mut self.oracle,
            )?;
            Ok(())
        }
    }

    fn emit_event(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        topics: &arrayvec::ArrayVec<
            <Self::IOTypes as SystemIOTypesConfig>::EventKey,
            MAX_EVENT_TOPICS,
        >,
        data: &[u8],
    ) -> Result<(), SystemError> {
        // Charge resources
        let ergs = match ee_type {
            ExecutionEnvironmentType::EVM => {
                let static_cost = LOG;
                let topic_cost = LOGTOPIC * (topics.len() as u64);
                let len_cost = (data.len() as u64) * LOGDATA;
                let cost = static_cost + topic_cost + len_cost;
                let ergs = cost.checked_mul(ERGS_PER_GAS).ok_or(out_of_ergs_error!())?;
                Ergs(ergs)
            }
            _ => return Err(internal_error!("Unsupported EE").into()),
        };
        let native = R::Native::from_computational(
            EVENT_STORAGE_BASE_NATIVE_COST
                + EVENT_TOPIC_NATIVE_COST * (topics.len() as u64)
                + EVENT_DATA_PER_BYTE_COST * (data.len() as u64),
        );
        resources.charge(&R::from_ergs_and_native(ergs, native))?;

        let data = UsizeAlignedByteBox::from_slice_in(data, self.allocator.clone());
        self.events_storage
            .push_event(self.tx_number, address, topics, data)
    }

    fn emit_l1_message(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        data: &[u8],
    ) -> Result<Bytes32, SystemError> {
        // TODO(EVM-1077): consider adding COMPUTATIONAL_PRICE_FOR_PUBDATA as in Era

        // We need to charge cost of hashing:
        // - keccak256_native_cost(L2_TO_L1_LOG_SERIALIZE_SIZE) and
        //   keccak256_native_cost(64) when reconstructing L2ToL1Log
        // - keccak256_native_cost(64) + keccak256_native_cost(data.len())
        //   when reconstructing Messages
        // - at most 1 time keccak256_native_cost(64) when building the
        //   Merkle tree (as merkle tree can contain ~2*N nodes, where the
        //   first N nodes are leaves the hash of which is calculated on the
        //   previous step).

        let hashing_native_cost =
            keccak256_native_cost::<Self::Resources>(L2_TO_L1_LOG_SERIALIZE_SIZE).as_u64()
                + 3 * keccak256_native_cost::<Self::Resources>(64).as_u64()
                + keccak256_native_cost::<Self::Resources>(data.len()).as_u64();

        // We also charge some native resource for storing the log
        let native = R::Native::from_computational(
            hashing_native_cost
                + EVENT_STORAGE_BASE_NATIVE_COST
                + EVENT_DATA_PER_BYTE_COST * (data.len() as u64),
        );
        resources.charge(&R::from_native(native))?;

        // TODO(EVM-1078): for Era backward compatibility we may need to add events for l2 to l1 log and l1 message

        let mut data_hash = ArrayBuilder::default();
        Keccak256Impl::execute(&data, &mut data_hash, resources, self.allocator.clone()).map_err(
            |e| match e {
                SystemFunctionError::InvalidInput => unreachable!(),
                SystemFunctionError::System(e) => e,
            },
        )?;
        let data_hash = Bytes32::from_array(data_hash.build());
        let data = UsizeAlignedByteBox::from_slice_in(data, self.allocator.clone());
        self.logs_storage
            .push_message(self.tx_number, address, data, data_hash)?;
        Ok(data_hash)
    }

    fn get_nominal_token_balance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue, SystemError> {
        self.storage
            .read_account_properties(
                ee_type,
                resources,
                address,
                AccountDataRequest::empty().with_nominal_token_balance(),
                &mut self.oracle,
            )
            .map(|account_data| account_data.nominal_token_balance.0)
    }

    fn get_observable_bytecode(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<&'static [u8], SystemError> {
        // TODO(EVM-1079): separate observable and usable better
        self.storage
            .read_account_properties(
                ee_type,
                resources,
                address,
                AccountDataRequest::empty().with_bytecode(),
                &mut self.oracle,
            )
            .map(|account_data| account_data.bytecode.0)
    }

    fn get_observable_bytecode_hash(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue, SystemError> {
        let AccountData {
            observable_bytecode_hash,
            nominal_token_balance,
            nonce,
            ..
        } = self.storage.read_account_properties(
            ee_type,
            resources,
            address,
            AccountDataRequest::empty()
                .with_observable_bytecode_hash()
                .with_nominal_token_balance()
                .with_nonce(),
            &mut self.oracle,
        )?;
        Ok(
            if observable_bytecode_hash.0.is_zero() && ee_type == ExecutionEnvironmentType::EVM {
                // It is extremely unlikely that a hash is zero, so we can assume
                // that it is an EOA or an empty account

                // Here we know that code is empty, we consider the account to be empty
                // if balance and nonce are 0.
                let empty_acc = nonce.0 == 0 && nominal_token_balance.0.is_zero();

                if empty_acc {
                    Bytes32::ZERO
                } else {
                    // EOA case:
                    Bytes32::from_u256_be(&U256::from_limbs([
                        0x7bfad8045d85a470,
                        0xe500b653ca82273b,
                        0x927e7db2dcc703c0,
                        0xc5d2460186f7233c,
                    ]))
                }
            } else {
                observable_bytecode_hash.0
            },
        )
    }

    fn get_observable_bytecode_size(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<u32, SystemError> {
        self.storage
            .read_account_properties(
                ee_type,
                resources,
                address,
                AccountDataRequest::empty().with_observable_bytecode_len(),
                &mut self.oracle,
            )
            .map(|account_data| account_data.observable_bytecode_len.0)
    }

    fn get_selfbalance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue, SystemError> {
        self.storage.get_selfbalance(ee_type, resources, address)
    }

    fn mark_for_deconstruction(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        nominal_token_beneficiary: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        in_constructor: bool,
    ) -> Result<(), SystemError> {
        self.storage.mark_for_deconstruction(
            from_ee,
            resources,
            at_address,
            nominal_token_beneficiary,
            &mut self.oracle,
            in_constructor,
        )
    }

    fn net_pubdata_used(&self) -> Result<u64, InternalError> {
        Ok(self.storage.pubdata_used_by_tx() as u64
            + self.logs_storage.calculate_pubdata_used_by_tx()? as u64)
    }

    fn start_io_frame(&mut self) -> Result<FullIOStateSnapshot, InternalError> {
        let io = self.storage.start_frame();
        let transient = self.transient_storage.start_frame();
        let messages = self.logs_storage.start_frame();
        let events = self.events_storage.start_frame();

        Ok(FullIOStateSnapshot {
            io,
            transient,
            messages,
            events,
        })
    }

    fn finish_io_frame(
        &mut self,
        rollback_handle: Option<&FullIOStateSnapshot>,
    ) -> Result<(), InternalError> {
        self.storage.finish_frame(rollback_handle.map(|x| &x.io))?;
        self.transient_storage
            .finish_frame(rollback_handle.map(|x| &x.transient))?;
        self.logs_storage
            .finish_frame(rollback_handle.map(|x| x.messages));
        self.events_storage
            .finish_frame(rollback_handle.map(|x| x.events));

        Ok(())
    }
}

pub trait FinishIO {
    type FinalData;

    fn finish(
        self,
        block_metadata: BlockMetadataFromOracle,
        current_block_hash: Bytes32,
        l1_to_l2_txs_hash: Bytes32,
        upgrade_tx_hash: Bytes32,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        logger: impl Logger,
    ) -> Self::FinalData;
}

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        O: IOOracle,
    > FinishIO for FullIO<A, R, P, SC, SCC, O, false>
where
    ExtraCheck<SCC, A>:,
{
    type FinalData = O;
    fn finish(
        mut self,
        _block_metadata: BlockMetadataFromOracle,
        current_block_hash: Bytes32,
        _l1_to_l2_txs_hash: Bytes32,
        _upgrade_tx_hash: Bytes32,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        mut logger: impl Logger,
    ) -> Self::FinalData {
        result_keeper.pubdata(current_block_hash.as_u8_ref());
        // dump pubdata and state diffs
        self.storage
            .finish(
                &mut self.oracle,
                // no storage commitment
                None,
                // we don't need to append pubdata to the hash
                &mut NopHasher,
                result_keeper,
                &mut logger,
            )
            .expect("Failed to finish storage");
        self.logs_storage
            .apply_pubdata(&mut NopHasher, result_keeper);
        result_keeper.logs(self.logs_storage.messages_ref_iter());
        result_keeper.events(self.events_storage.events_ref_iter());

        self.oracle
    }
}

// In practice we will not use single block batches
// This functionality is here only for the tests
#[cfg(not(feature = "wrap-in-batch"))]
impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        O: IOOracle,
    > FinishIO for FullIO<A, R, P, SC, SCC, O, true>
where
    ExtraCheck<SCC, A>:,
{
    type FinalData = (O, Bytes32);
    fn finish(
        mut self,
        block_metadata: BlockMetadataFromOracle,
        current_block_hash: Bytes32,
        l1_to_l2_txs_hash: Bytes32,
        upgrade_tx_hash: Bytes32,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        mut logger: impl Logger,
    ) -> Self::FinalData {
        let mut state_commitment = {
            let mut initialization_iterator = self
                .oracle
                .create_oracle_access_iterator::<InitializeIOImplementerIterator>(())
                .unwrap();
            // TODO (EVM-989): read only state commitment
            let fsm_state =
                <BasicIOImplementerFSM::<FlatStorageCommitment<TREE_HEIGHT>> as UsizeDeserializable>::from_iter(&mut initialization_iterator).unwrap();
            assert_eq!(initialization_iterator.len(), 0);
            fsm_state.state_root_view
        };

        let mut blocks_hasher = Blake2s256::new();
        for block_hash in block_metadata.block_hashes.0.iter() {
            blocks_hasher.update(&block_hash.to_be_bytes::<32>());
        }

        // chain state before
        let chain_state_commitment_before = ChainStateCommitment {
            state_root: state_commitment.root,
            next_free_slot: state_commitment.next_free_slot,
            block_number: block_metadata.block_number - 1,
            last_256_block_hashes_blake: blocks_hasher.finalize().into(),
            // TODO(EVM-1080): we should set and validate that current block timestamp >= previous
            last_block_timestamp: 0,
        };

        // finishing IO, applying changes
        let mut pubdata_hasher = Blake2s256::new();
        pubdata_hasher.update(current_block_hash.as_u8_ref());
        let mut l2_to_l1_logs_hasher = Blake2s256::new();

        self.storage
            .finish(
                &mut self.oracle,
                Some(&mut state_commitment),
                &mut pubdata_hasher,
                result_keeper,
                &mut logger,
            )
            .expect("Failed to finish storage");
        self.logs_storage
            .apply_l2_to_l1_logs_hashes_to_hasher(&mut l2_to_l1_logs_hasher);
        self.logs_storage
            .apply_pubdata(&mut pubdata_hasher, result_keeper);
        result_keeper.logs(self.logs_storage.messages_ref_iter());
        result_keeper.events(self.events_storage.events_ref_iter());

        let pubdata_hash = pubdata_hasher.finalize();
        let l2_to_l1_logs_hashes_hash = l2_to_l1_logs_hasher.finalize();

        blocks_hasher = Blake2s256::new();
        for block_hash in block_metadata.block_hashes.0.iter().skip(1) {
            blocks_hasher.update(&block_hash.to_be_bytes::<32>());
        }
        blocks_hasher.update(current_block_hash.as_u8_ref());

        // chain state after
        let chain_state_commitment_after = ChainStateCommitment {
            state_root: state_commitment.root,
            next_free_slot: state_commitment.next_free_slot,
            block_number: block_metadata.block_number,
            last_256_block_hashes_blake: blocks_hasher.finalize().into(),
            // TODO(EVM-1080): we should set and validate that current block timestamp >= previous
            last_block_timestamp: 0,
        };

        // other outputs to be opened on the settlement layer/aggregation program
        let block_output = BlocksOutput {
            chain_id: U256::try_from(block_metadata.chain_id).unwrap(),
            first_block_timestamp: block_metadata.timestamp,
            last_block_timestamp: block_metadata.timestamp,
            pubdata_hash: pubdata_hash.into(),
            priority_ops_hashes_hash: l1_to_l2_txs_hash,
            l2_to_l1_logs_hashes_hash: l2_to_l1_logs_hashes_hash.into(),
            upgrade_tx_hash,
        };

        let public_input = BlocksPublicInput {
            state_before: chain_state_commitment_before.hash().into(),
            state_after: chain_state_commitment_after.hash().into(),
            blocks_output: block_output.hash().into(),
        };

        (self.oracle, public_input.hash().into())
    }
}

#[cfg(feature = "wrap-in-batch")]
impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        O: IOOracle,
    > FinishIO for FullIO<A, R, P, SC, SCC, O, true>
where
    ExtraCheck<SCC, A>:,
{
    type FinalData = (O, Bytes32);
    fn finish(
        mut self,
        block_metadata: BlockMetadataFromOracle,
        current_block_hash: Bytes32,
        _l1_to_l2_txs_hash: Bytes32,
        upgrade_tx_hash: Bytes32,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        mut logger: impl Logger,
    ) -> Self::FinalData {
        let mut state_commitment = {
            let mut initialization_iterator = self
                .oracle
                .create_oracle_access_iterator::<InitializeIOImplementerIterator>(())
                .unwrap();
            // TODO (EVM-989): read only state commitment
            let fsm_state =
                <BasicIOImplementerFSM::<FlatStorageCommitment<TREE_HEIGHT>> as UsizeDeserializable>::from_iter(&mut initialization_iterator).unwrap();
            assert_eq!(initialization_iterator.len(), 0);
            fsm_state.state_root_view
        };

        // chain state before
        // currently we generate simplified commitment(only to state) for tests.
        let _ = logger.write_fmt(format_args!(
            "PI calculation: state commitment before {:?}\n",
            state_commitment
        ));
        let mut chain_state_hasher = Blake2s256::new();
        chain_state_hasher.update(state_commitment.root.as_u8_ref());
        chain_state_hasher.update(state_commitment.next_free_slot.to_be_bytes());
        let chain_state_commitment_before = chain_state_hasher.finalize();

        // finishing IO, applying changes
        let mut pubdata_hasher = crypto::sha3::Keccak256::new();
        pubdata_hasher.update(current_block_hash.as_u8_ref());

        self.storage
            .finish(
                &mut self.oracle,
                Some(&mut state_commitment),
                &mut pubdata_hasher,
                result_keeper,
                &mut logger,
            )
            .expect("Failed to finish storage");

        self.logs_storage
            .apply_pubdata(&mut pubdata_hasher, result_keeper);
        result_keeper.logs(self.logs_storage.messages_ref_iter());
        result_keeper.events(self.events_storage.events_ref_iter());
        let mut full_root_hasher = crypto::sha3::Keccak256::new();
        full_root_hasher.update(self.logs_storage.tree_root().as_u8_ref());
        full_root_hasher.update([0u8; 32]); // aggregated root 0 for now
        let full_l2_to_l1_logs_root = full_root_hasher.finalize();
        let l1_txs_commitment = self.logs_storage.l1_txs_commitment();

        let pubdata_hash = pubdata_hasher.finalize();

        // chain state after
        // currently we generate simplified commitment(only to state) for tests.
        let _ = logger.write_fmt(format_args!(
            "PI calculation: state commitment after {:?}\n",
            state_commitment
        ));
        let mut chain_state_hasher = Blake2s256::new();
        chain_state_hasher.update(state_commitment.root.as_u8_ref());
        chain_state_hasher.update(state_commitment.next_free_slot.to_be_bytes());
        let chain_state_commitment_after = chain_state_hasher.finalize();

        let mut da_commitment_hasher = crypto::sha3::Keccak256::new();
        da_commitment_hasher.update([0u8; 32]); // we don't have to validate state diffs hash
        da_commitment_hasher.update(pubdata_hash); // full pubdata keccak
        da_commitment_hasher.update([1u8]); // with calldata we should provide 1 blob
        da_commitment_hasher.update([0u8; 32]); // its hash will be ignored on the settlement layer
        let da_commitment = da_commitment_hasher.finalize();
        let batch_output = public_input::BatchOutput {
            chain_id: U256::try_from(block_metadata.chain_id).unwrap(),
            first_block_timestamp: block_metadata.timestamp,
            last_block_timestamp: block_metadata.timestamp,
            used_l2_da_validator_address: ruint::aliases::B160::ZERO,
            pubdata_commitment: da_commitment.into(),
            number_of_layer_1_txs: U256::try_from(l1_txs_commitment.0).unwrap(),
            priority_operations_hash: l1_txs_commitment.1,
            l2_logs_tree_root: full_l2_to_l1_logs_root.into(),
            upgrade_tx_hash,
        };
        let _ = logger.write_fmt(format_args!(
            "PI calculation: batch output {:?}\n",
            batch_output,
        ));

        let public_input = public_input::BatchPublicInput {
            state_before: chain_state_commitment_before.into(),
            state_after: chain_state_commitment_after.into(),
            batch_output: batch_output.hash().into(),
        };
        let _ = logger.write_fmt(format_args!(
            "PI calculation: final batch public input {:?}\n",
            public_input,
        ));
        let public_input_hash = public_input.hash().into();
        let _ = logger.write_fmt(format_args!(
            "PI calculation: final batch public input hash {:?}\n",
            public_input_hash,
        ));

        (self.oracle, public_input_hash)
    }
}

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        O: IOOracle,
        const PROOF_ENV: bool,
    > IOSubsystemExt for FullIO<A, R, P, SC, SCC, O, PROOF_ENV>
where
    ExtraCheck<SCC, A>:,
    Self: FinishIO,
{
    type IOOracle = O;
    type FinalData = <Self as FinishIO>::FinalData;

    fn init_from_oracle(oracle: Self::IOOracle) -> Result<Self, InternalError> {
        let allocator = A::default();

        let storage =
            FlatTreeWithAccountsUnderHashesStorageModel::construct(P::default(), allocator.clone());

        let transient_storage =
            GenericTransientStorage::<WarmStorageKey, Bytes32, SC, SCC, A>::new_from_parts(
                allocator.clone(),
            );
        let logs_storage = LogsStorage::<SC, SCC, A>::new_from_parts(allocator.clone());
        let events_storage =
            EventsStorage::<MAX_EVENT_TOPICS, SC, SCC, A>::new_from_parts(allocator.clone());

        let new = Self {
            storage,
            transient_storage,
            events_storage,
            logs_storage,
            allocator,
            oracle,
            tx_number: 0u32,
        };

        Ok(new)
    }

    fn oracle(&mut self) -> &mut Self::IOOracle {
        &mut self.oracle
    }

    fn begin_next_tx(&mut self) {
        self.storage.begin_new_tx();
        self.transient_storage.begin_new_tx();
        self.logs_storage.begin_new_tx();
        self.events_storage.begin_new_tx();
    }

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        self.storage.finish_tx()?;
        self.tx_number += 1;
        Ok(())
    }

    fn storage_touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        is_access_list: bool,
    ) -> Result<(), SystemError> {
        self.storage.storage_touch(
            ee_type,
            resources,
            address,
            key,
            &mut self.oracle,
            is_access_list,
        )
    }

    fn read_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<u64, SystemError> {
        self.storage
            .read_account_properties(
                ee_type,
                resources,
                address,
                AccountDataRequest::empty().with_nonce(),
                &mut self.oracle,
            )
            .map(|account_data| account_data.nonce.0)
    }

    fn increment_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        increment_by: u64,
    ) -> Result<u64, UpdateQueryError> {
        self.storage
            .increment_nonce(ee_type, resources, address, increment_by, &mut self.oracle)
    }

    fn touch_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        is_access_list: bool,
    ) -> Result<(), SystemError> {
        self.storage.touch_account(
            ee_type,
            resources,
            address,
            &mut self.oracle,
            is_access_list,
        )
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
            >,
        >,
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
        self.storage
            .read_account_properties(ee_type, resources, address, request, &mut self.oracle)
    }

    fn transfer_nominal_token_value(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        from: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        to: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        amount: &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    ) -> Result<(), UpdateQueryError> {
        self.storage.transfer_nominal_token_value(
            ee_type,
            resources,
            from,
            to,
            amount,
            &mut self.oracle,
        )
    }

    fn deploy_code(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        bytecode: &[u8],
        bytecode_len: u32,
        artifacts_len: u32,
    ) -> Result<&'static [u8], SystemError> {
        self.storage.deploy_code(
            from_ee,
            resources,
            at_address,
            bytecode,
            bytecode_len,
            artifacts_len,
            &mut self.oracle,
        )
    }

    fn set_bytecode_details(
        &mut self,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        ee: ExecutionEnvironmentType,
        bytecode_hash: Bytes32,
        bytecode_len: u32,
        artifacts_len: u32,
        observable_bytecode_hash: Bytes32,
        observable_bytecode_len: u32,
    ) -> Result<(), SystemError> {
        self.storage.set_bytecode_details(
            resources,
            at_address,
            ee,
            bytecode_hash,
            bytecode_len,
            artifacts_len,
            observable_bytecode_hash,
            observable_bytecode_len,
            &mut self.oracle,
        )
    }

    fn finish(
        self,
        block_metadata: BlockMetadataFromOracle,
        current_block_hash: Bytes32,
        l1_to_l2_txs_hash: Bytes32,
        upgrade_tx_hash: Bytes32,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        logger: impl Logger,
    ) -> Self::FinalData {
        FinishIO::finish(
            self,
            block_metadata,
            current_block_hash,
            l1_to_l2_txs_hash,
            upgrade_tx_hash,
            result_keeper,
            logger,
        )
    }

    fn emit_l1_l2_tx_log(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _resources: &mut Self::Resources,
        tx_hash: Bytes32,
        success: bool,
    ) -> Result<(), SystemError> {
        // Resources for it charged as part of intrinsic
        self.logs_storage
            .push_l1_l2_tx_log(self.tx_number, tx_hash, success)
    }

    fn update_account_nominal_token_balance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        diff: &ruint::aliases::U256,
        should_subtract: bool,
    ) -> Result<ruint::aliases::U256, UpdateQueryError> {
        let update_fn = move |old_value: &ruint::aliases::U256| {
            let new_value = if should_subtract {
                old_value.checked_sub(*diff)
            } else {
                old_value.checked_add(*diff)
            };
            new_value.ok_or(UpdateQueryError::NumericBoundsError)
        };
        self.storage.update_nominal_token_value(
            ee_type,
            resources,
            address,
            update_fn,
            &mut self.oracle,
        )
    }
}

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        O: IOOracle,
        const PROOF_ENV: bool,
    > EthereumLikeIOSubsystem for FullIO<A, R, P, SC, SCC, O, PROOF_ENV>
where
    ExtraCheck<SCC, A>:,
{
}
