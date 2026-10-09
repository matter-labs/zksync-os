//! The IO subsystem of a transaction range: a copy of `FullIO` specialized to
//! the access-list storage model, without the L1 messages, interop roots and
//! settlement-layer storages that an Ethereum block has no use for.
//!
//! Initialization witnesses the access list, the initial value tables and the
//! range (see `witness`), builds the storage model, and keeps the hashes for
//! the pre-transaction op to check against the header.
//!
//! Every value transfer (a call, a deployment, a self-destruct sweep, the
//! transaction's own) emits the transfer log of EIP-7708 from the system
//! address, as it goes through this IO.

use super::storage_model::{BalModelSnapshot, BalStorageModel};
use super::witness::{
    witness_transaction_count, witness_transaction_range, BlockAccessIndexRange, InitialValues,
    TransactionRange,
};
use crate::bootloader::block_flow::ethereum_glamsterdam::oracle_queries::{
    ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_DATA_QUERY_ID, ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_LEN_QUERY_ID,
};
use basic_system::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use core::alloc::Allocator;
use evm_interpreter::gas_constants::{LOG, LOGDATA, LOGTOPIC, TLOAD, TSTORE};
use ruint::aliases::{B160, U256};
use storage_models::common_structs::generic_transient_storage::GenericTransientStorage;
use storage_models::common_structs::snapshottable_io::SnapshottableIo;
use storage_models::common_structs::StorageModel;
use zk_ee::common_structs::events_storage::{
    EventsStorage, GenericEventContentRef, GenericEventContentWithTxRef,
};
use zk_ee::common_structs::history_map::CacheSnapshotId;
use zk_ee::common_structs::logs_storage::GenericLogContentWithTxRef;
use zk_ee::common_structs::WarmStorageKey;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::memory_io::get_bytes_with_length_query;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::{
    AccountData, AccountDataRequest, BalanceError, BalanceSubsystemError,
    DeconstructionSubsystemError, EthereumLikeIOSubsystem, IOResultKeeper, IOSubsystem,
    IOSubsystemExt, IOTeardown, Maybe, NonceSubsystemError, StateGasResources, MAX_EVENT_TOPICS,
};
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;
use zk_ee::{interface_error, internal_error};

use basic_system::cost_constants::{BLAKE2S_CHUNK_SIZE, BLAKE2S_ROUND_NATIVE_COST};
use basic_system::system_implementation::flat_storage_model::cost_constants::{
    EVENT_DATA_PER_BYTE_COST, EVENT_STORAGE_BASE_NATIVE_COST, EVENT_TOPIC_NATIVE_COST,
    RECEIPT_LOG_RLP_OVERHEAD_BYTES, WARM_TSTORAGE_READ_NATIVE_COST,
    WARM_TSTORAGE_WRITE_NATIVE_COST,
};
use zk_ee::common_structs::interop_root_storage::InteropRoot;
use zk_ee::utils::UsizeAlignedByteBox;

/// The address the transfer logs of EIP-7708 are emitted from
pub const SYSTEM_ADDRESS: B160 =
    B160::from_limbs([0xfffffffffffffffe, 0xffffffffffffffff, 0xffffffff]);
/// `keccak256("Transfer(address,address,uint256)")`, the first topic of a transfer log
pub const TRANSFER_TOPIC: Bytes32 =
    Bytes32::from_hex("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

/// A left-padded address, as a log topic
fn address_topic(address: &B160) -> Bytes32 {
    let mut topic = [0u8; 32];
    topic[12..].copy_from_slice(&address.to_be_bytes::<{ B160::BYTES }>());
    Bytes32::from_array(topic)
}

pub struct GlamsterdamIO<
    A: Allocator + Clone + Default,
    R: StateGasResources,
    P: StorageAccessPolicy<R, Bytes32> + Default,
    SF: StackFactory<N>,
    const N: usize,
    O: IOOracle,
    const PROOF_ENV: bool,
> {
    pub storage: BalStorageModel<A, R, P, PROOF_ENV>,
    pub transient_storage: GenericTransientStorage<WarmStorageKey, Bytes32, SF, N, A>,
    pub events_storage: EventsStorage<MAX_EVENT_TOPICS, SF, N, A>,
    pub allocator: A,
    pub oracle: O,
    pub tx_number: u32,
    /// The transaction range being executed, as witnessed
    pub transactions: TransactionRange,
    /// The number of transactions of the block, as witnessed (unverified: the
    /// aggregation binds it to the pre-execution check's)
    pub transaction_count: u32,
}

pub struct GlamsterdamIOSnapshot<R> {
    io: BalModelSnapshot<R>,
    transient: CacheSnapshotId,
    events: usize,
}

impl<
        A: Allocator + Clone + Default,
        R: StateGasResources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SF: StackFactory<N>,
        const N: usize,
        O: IOOracle,
        const PROOF_ENV: bool,
    > GlamsterdamIO<A, R, P, SF, N, O, PROOF_ENV>
{
    /// The block access indices the range covers
    pub fn range(&self) -> BlockAccessIndexRange {
        self.storage.range
    }

    /// The transaction range being executed
    pub fn transactions(&self) -> TransactionRange {
        self.transactions
    }

    /// The number of transactions of the block, as witnessed
    pub fn transaction_count(&self) -> u32 {
        self.transaction_count
    }

    /// Starts an access index (the pre-transaction phase, a transaction, or
    /// the post-transaction phase)
    pub fn begin_access_index(&mut self, index: u32) -> Result<(), InternalError> {
        self.storage.begin_index(index)?;
        self.transient_storage.begin_new_tx();
        self.events_storage.begin_new_tx();
        Ok(())
    }

    /// Ends an access index: the writes are checked against the access list
    pub fn finish_access_index(&mut self) -> Result<(), InternalError> {
        self.storage.finish_tx()
    }

    /// The transfer log of EIP-7708: nothing for a zero amount or a transfer to self
    fn emit_transfer_log(
        &mut self,
        resources: &mut R,
        from: &B160,
        to: &B160,
        amount: &U256,
    ) -> Result<(), SystemError> {
        if amount.is_zero() || from == to {
            return Ok(());
        }
        let mut topics = arrayvec::ArrayVec::<Bytes32, MAX_EVENT_TOPICS>::new();
        topics.push(TRANSFER_TOPIC);
        topics.push(address_topic(from));
        topics.push(address_topic(to));
        let data = amount.to_be_bytes::<32>();
        // no gas: the transfer paid for it; the native of the log is charged
        self.emit_event(
            ExecutionEnvironmentType::NoEE,
            resources,
            &SYSTEM_ADDRESS,
            &topics,
            &data,
        )
    }
}

impl<
        A: Allocator + Clone + Default,
        R: StateGasResources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SF: StackFactory<N>,
        const N: usize,
        O: IOOracle,
        const PROOF_ENV: bool,
    > IOSubsystem for GlamsterdamIO<A, R, P, SF, N, O, PROOF_ENV>
{
    type IOTypes = EthereumIOTypesConfig;
    type Resources = R;
    type StateSnapshot = GlamsterdamIOSnapshot<R>;

    const STORAGE_SLOTS_LE: bool = false;

    fn storage_read<const TRANSIENT: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        key: &Bytes32,
    ) -> Result<Bytes32, SystemError> {
        let mut result = Bytes32::ZERO;
        self.storage_read_and_place::<TRANSIENT>(ee_type, resources, address, key, |value| {
            result = *value
        })?;
        Ok(result)
    }

    fn storage_read_and_place<const TRANSIENT: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        key: &Bytes32,
        place: impl FnOnce(&Bytes32),
    ) -> Result<(), SystemError> {
        self.storage_read_raw_and_place::<TRANSIENT>(ee_type, resources, address, key, place)
    }

    fn storage_read_raw_and_place<const TRANSIENT: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        key: &Bytes32,
        place: impl FnOnce(&Bytes32),
    ) -> Result<(), SystemError> {
        if TRANSIENT {
            let gas = match ee_type {
                ExecutionEnvironmentType::NoEE => 0,
                ExecutionEnvironmentType::EVM => TLOAD,
            };
            resources.charge_legacy_gas_and_native(gas, WARM_TSTORAGE_READ_NATIVE_COST)?;
            let key = WarmStorageKey {
                address: *address,
                key: *key,
            };
            self.transient_storage.apply_read_with(&key, place)
        } else {
            self.storage.storage_read_and_place(
                ee_type,
                resources,
                address,
                key,
                &mut self.oracle,
                place,
            )
        }
    }

    fn storage_write<const TRANSIENT: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        key: &Bytes32,
        value_to_write: &Bytes32,
    ) -> Result<(), SystemError> {
        self.storage_write_raw::<TRANSIENT>(ee_type, resources, address, key, value_to_write)
    }

    fn storage_write_raw<const TRANSIENT: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        key: &Bytes32,
        value_to_write: &Bytes32,
    ) -> Result<(), SystemError> {
        if TRANSIENT {
            let gas = match ee_type {
                ExecutionEnvironmentType::NoEE => 0,
                ExecutionEnvironmentType::EVM => TSTORE,
            };
            resources.charge_legacy_gas_and_native(gas, WARM_TSTORAGE_WRITE_NATIVE_COST)?;
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
        resources: &mut R,
        address: &B160,
        topics: &arrayvec::ArrayVec<Bytes32, MAX_EVENT_TOPICS>,
        data: &[u8],
    ) -> Result<(), SystemError> {
        let gas = match ee_type {
            ExecutionEnvironmentType::NoEE => 0,
            ExecutionEnvironmentType::EVM => {
                LOG as u64
                    + LOGTOPIC as u64 * topics.len() as u64
                    + LOGDATA as u64 * data.len() as u64
            }
        };
        // the same native charge as `FullIO`: the receipt hashing of the log
        let receipt_rlp_len_upper_bound = RECEIPT_LOG_RLP_OVERHEAD_BYTES
            .saturating_add(33 * topics.len() as u64)
            .saturating_add(data.len() as u64);
        let receipt_hash_native = receipt_rlp_len_upper_bound
            .div_ceil(BLAKE2S_CHUNK_SIZE as u64)
            .saturating_mul(BLAKE2S_ROUND_NATIVE_COST);
        let native = EVENT_STORAGE_BASE_NATIVE_COST
            + EVENT_TOPIC_NATIVE_COST * topics.len() as u64
            + EVENT_DATA_PER_BYTE_COST * data.len() as u64
            + receipt_hash_native;
        resources.charge_legacy_gas_and_native(zk_ee::system::legacy_gas_u32(gas)?, native)?;
        let data = UsizeAlignedByteBox::from_slice_in(data, self.allocator.clone());
        self.events_storage
            .push_event(self.tx_number, address, topics, data)
    }

    fn emit_l1_message(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _resources: &mut R,
        _address: &B160,
        _data: &[u8],
    ) -> Result<Bytes32, SystemError> {
        Err(internal_error!("no L1 messages in an Ethereum block").into())
    }

    fn emit_interop_commitment_leaf(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _resources: &mut R,
        _leaf_hash: Bytes32,
    ) -> Result<(), SystemError> {
        Err(internal_error!("no interop in an Ethereum block").into())
    }

    fn add_interop_root(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _resources: &mut R,
        _interop_root: InteropRoot,
    ) -> Result<(), SystemError> {
        Err(internal_error!("no interop in an Ethereum block").into())
    }

    fn update_settlement_layer_chain_id(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _resources: &mut R,
        _new_chain_id: U256,
    ) -> Result<(), SystemError> {
        Err(internal_error!("no settlement layer in an Ethereum block").into())
    }

    fn get_nominal_token_balance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
    ) -> Result<U256, SystemError> {
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
        resources: &mut R,
        address: &B160,
    ) -> Result<&'static [u8], SystemError> {
        self.storage
            .read_account_properties(
                ee_type,
                resources,
                address,
                AccountDataRequest::empty()
                    .with_bytecode()
                    .with_unpadded_code_len(),
                &mut self.oracle,
            )
            .map(|account_data| {
                account_data.bytecode.0[..account_data.unpadded_code_len.0 as usize].as_ref()
            })
    }

    fn get_observable_bytecode_hash(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
    ) -> Result<Bytes32, SystemError> {
        self.storage
            .read_account_properties(
                ee_type,
                resources,
                address,
                AccountDataRequest::empty().with_observable_bytecode_hash(),
                &mut self.oracle,
            )
            .map(|account_data| account_data.observable_bytecode_hash.0)
    }

    fn get_observable_bytecode_size(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
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
        resources: &mut R,
        address: &B160,
    ) -> Result<U256, SystemError> {
        self.storage.get_selfbalance(ee_type, resources, address)
    }

    fn mark_for_deconstruction(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: &B160,
        nominal_token_beneficiary: &B160,
    ) -> Result<U256, DeconstructionSubsystemError> {
        let amount = self.storage.mark_for_deconstruction(
            from_ee,
            resources,
            at_address,
            nominal_token_beneficiary,
            &mut self.oracle,
        )?;
        self.emit_transfer_log(resources, at_address, nominal_token_beneficiary, &amount)?;
        Ok(amount)
    }

    fn net_pubdata_used(&self) -> Result<u64, InternalError> {
        Ok(0)
    }

    fn net_committed_log_records_pubdata_used(&self) -> Result<u64, InternalError> {
        Ok(0)
    }

    fn start_io_frame(&mut self) -> Result<Self::StateSnapshot, InternalError> {
        Ok(GlamsterdamIOSnapshot {
            io: self.storage.start_frame(),
            transient: self.transient_storage.start_frame(),
            events: self.events_storage.start_frame(),
        })
    }

    fn finish_io_frame(
        &mut self,
        rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError> {
        self.storage.finish_frame(rollback_handle.map(|x| &x.io))?;
        self.transient_storage
            .finish_frame(rollback_handle.map(|x| &x.transient))?;
        self.events_storage
            .finish_frame(rollback_handle.map(|x| x.events));
        Ok(())
    }

    fn increment_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        increment_by: u64,
    ) -> Result<u64, NonceSubsystemError> {
        self.storage
            .increment_nonce(ee_type, resources, address, increment_by, &mut self.oracle)
    }

    fn read_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
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

    fn get_refund_counter(&'_ self) -> &'_ R {
        self.storage.get_refund_counter()
    }
}

impl<
        A: Allocator + Clone + Default,
        R: StateGasResources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SF: StackFactory<N>,
        const N: usize,
        O: IOOracle,
        const PROOF_ENV: bool,
    > IOSubsystemExt for GlamsterdamIO<A, R, P, SF, N, O, PROOF_ENV>
{
    type IOOracle = O;

    fn init_from_oracle(mut oracle: Self::IOOracle) -> Result<Self, InternalError> {
        let allocator = A::default();
        let encoding = get_bytes_with_length_query::<_, (), _>(
            &mut oracle,
            ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_LEN_QUERY_ID,
            ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_DATA_QUERY_ID,
            (),
            allocator.clone(),
        )?
        .ok_or(internal_error!("the block access list is empty"))?;
        let initial = InitialValues::witness(&mut oracle, allocator.clone())
            .map_err(|_| internal_error!("malformed initial value tables"))?;
        let transactions = witness_transaction_range(&mut oracle)
            .map_err(|_| internal_error!("no transaction range"))?;
        let transaction_count = witness_transaction_count(&mut oracle)
            .map_err(|_| internal_error!("no transaction count"))?;
        let range = BlockAccessIndexRange::of(transactions, transaction_count)
            .map_err(|_| internal_error!("the transaction range is not in the block"))?;
        let storage =
            BalStorageModel::new(encoding, initial, range, P::default(), allocator.clone())
                .map_err(|_| internal_error!("malformed block access list"))?;
        Ok(Self {
            storage,
            transient_storage: GenericTransientStorage::new_from_parts(allocator.clone()),
            events_storage: EventsStorage::new_from_parts(allocator.clone()),
            allocator,
            oracle,
            tx_number: 0,
            transactions,
            transaction_count,
        })
    }

    fn oracle(&mut self) -> &mut Self::IOOracle {
        &mut self.oracle
    }

    fn begin_next_tx(&mut self) {
        self.transient_storage.begin_new_tx();
        self.events_storage.begin_new_tx();
    }

    fn block_scoped_cache_limit_hit_for_current_tx(&self) -> bool {
        false
    }

    fn transaction_cache_memory_limit_hit_for_current_tx(&self) -> bool {
        false
    }

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        self.storage.finish_tx()?;
        self.tx_number += 1;
        Ok(())
    }

    fn storage_touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        key: &Bytes32,
    ) -> Result<(), SystemError> {
        self.storage
            .storage_touch(ee_type, resources, address, key, &mut self.oracle)
    }

    fn touch_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
    ) -> Result<(), SystemError> {
        self.storage
            .touch_account(ee_type, resources, address, &mut self.oracle)
    }

    fn read_account_properties<
        EEVersion: Maybe<u8>,
        ObservableBytecodeHash: Maybe<Bytes32>,
        ObservableBytecodeLen: Maybe<u32>,
        Nonce: Maybe<u64>,
        BytecodeHash: Maybe<Bytes32>,
        BytecodeLen: Maybe<u32>,
        ArtifactsLen: Maybe<u32>,
        NominalTokenBalance: Maybe<U256>,
        Bytecode: Maybe<&'static [u8]>,
        CodeVersion: Maybe<u8>,
        IsDelegated: Maybe<bool>,
    >(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
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
                IsDelegated,
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
            CodeVersion,
            IsDelegated,
        >,
        SystemError,
    > {
        self.storage
            .read_account_properties(ee_type, resources, address, request, &mut self.oracle)
    }

    fn transfer_nominal_token_value(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        from: &B160,
        to: &B160,
        amount: &U256,
    ) -> Result<(), BalanceSubsystemError> {
        self.storage.transfer_nominal_token_value(
            ee_type,
            resources,
            from,
            to,
            amount,
            &mut self.oracle,
        )?;
        Ok(self.emit_transfer_log(resources, from, to, amount)?)
    }

    fn deploy_code(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: &B160,
        bytecode: &[u8],
    ) -> Result<(&'static [u8], Bytes32, u32), SystemError> {
        self.storage
            .deploy_code(from_ee, resources, at_address, bytecode, &mut self.oracle)
    }

    fn set_bytecode_details(
        &mut self,
        resources: &mut R,
        at_address: &B160,
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

    fn set_delegation(
        &mut self,
        resources: &mut R,
        at_address: &B160,
        delegate: &B160,
    ) -> Result<(), SystemError> {
        self.storage
            .set_delegation(resources, at_address, delegate, &mut self.oracle)
    }

    fn emit_l1_l2_tx_log(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _resources: &mut R,
        _tx_hash: Bytes32,
        _success: bool,
    ) -> Result<(), SystemError> {
        Err(internal_error!("no L1 transactions in an Ethereum block").into())
    }

    fn update_account_nominal_token_balance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        diff: &U256,
        should_subtract: bool,
        fee_payment_in_simulation: bool,
    ) -> Result<U256, BalanceSubsystemError> {
        let update_fn = move |old_value: &U256| {
            if should_subtract {
                old_value
                    .checked_sub(*diff)
                    .ok_or(interface_error! {BalanceError::InsufficientBalance})
            } else {
                old_value
                    .checked_add(*diff)
                    .ok_or(interface_error! {BalanceError::Overflow})
            }
        };
        self.storage.update_nominal_token_value(
            ee_type,
            resources,
            address,
            update_fn,
            &mut self.oracle,
            fee_payment_in_simulation,
        )
    }

    fn logs_len(&self) -> u64 {
        0
    }

    fn add_to_refund_counter(&mut self, refund: R) -> Result<(), SystemError> {
        self.storage.add_to_refund_counter(refund)
    }
}

impl<
        A: Allocator + Clone + Default,
        R: StateGasResources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SF: StackFactory<N>,
        const N: usize,
        O: IOOracle,
        const PROOF_ENV: bool,
    > EthereumLikeIOSubsystem for GlamsterdamIO<A, R, P, SF, N, O, PROOF_ENV>
{
}

impl<
        A: Allocator + Clone + Default,
        R: StateGasResources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SF: StackFactory<N>,
        const N: usize,
        O: IOOracle,
        const PROOF_ENV: bool,
    > IOTeardown<EthereumIOTypesConfig> for GlamsterdamIO<A, R, P, SF, N, O, PROOF_ENV>
{
    type IOStateCommitment = Bytes32;

    fn flush_caches(&mut self, result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>) {
        self.storage.persist_caches(&mut self.oracle, result_keeper);
    }

    fn report_new_preimages(
        &mut self,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) {
        self.storage.report_new_preimages(result_keeper);
    }

    type StorageKey<'a>
        = WarmStorageKey
    where
        Self: 'a;

    type StorageDiff<'a>
        = zk_ee::system::StorageDiff<EthereumIOTypesConfig>
    where
        Self: 'a;

    fn get_storage_diff<'a>(&'a self, key: Self::StorageKey<'a>) -> Option<Self::StorageDiff<'a>> {
        self.storage.get_storage_diff(key)
    }

    fn storage_diffs_iterator<'a>(
        &'a self,
    ) -> impl ExactSizeIterator<Item = (Self::StorageKey<'a>, Self::StorageDiff<'a>)> + Clone {
        self.storage.storage_diffs_iterator()
    }

    #[allow(refining_impl_trait_reachable)]
    fn events_in_this_tx_iterator<'a>(
        &'a self,
    ) -> impl ExactSizeIterator<
        Item = GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
    > + Clone {
        self.events_storage.events_in_transaction_ref_iter()
    }

    #[allow(refining_impl_trait_reachable)]
    fn events_iterator<'a>(
        &'a self,
    ) -> impl ExactSizeIterator<
        Item = GenericEventContentWithTxRef<'a, { MAX_EVENT_TOPICS }, EthereumIOTypesConfig>,
    > + Clone {
        self.events_storage.events_ref_iter()
    }

    fn signals_iterator<'a>(
        &'a self,
    ) -> impl ExactSizeIterator<Item = GenericLogContentWithTxRef<'a, EthereumIOTypesConfig>> + Clone
    {
        core::iter::empty()
    }

    fn update_commitment(
        &mut self,
        state_commitment: Option<&mut Self::IOStateCommitment>,
        logger: &mut impl Logger,
        result_keeper: &mut impl IOResultKeeper<Self::IOTypes>,
    ) {
        self.storage
            .update_commitment(state_commitment, &mut self.oracle, logger, result_keeper);
    }
}
