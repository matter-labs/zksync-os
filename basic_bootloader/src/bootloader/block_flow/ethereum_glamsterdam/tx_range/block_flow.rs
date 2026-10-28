//! The block flow of a transaction range `start..end` (see
//! `witness::TransactionRange`): the pre-transaction op witnesses the block's
//! accounting at the start of the range (`witness::BlockProgress`: the
//! receipts' cumulative gas, the gas sums and the logs bloom after the
//! transactions before it; nothing is read from their receipts) and runs the
//! system calls of the pre-transaction phase when the range starts at
//! transaction 0; the loop op
//! runs the transactions of the range, each from its transactions trie leaf,
//! checking its receipt and its state changes against the block access list;
//! the post-transaction op runs the withdrawals and the request system calls
//! (EIP-7002, EIP-7251 and the EIP-8282 builder requests) when the range ends
//! at the transaction count, carries the hashing of the deposit requests
//! across the ranges (a sha256 state witnessed at the start, the range's
//! deposit events absorbed after its transactions; see `requests`), checks
//! the header's requests hash in the range that ends the block, and reports
//! what the range established ([`GlamsterdamRangeOutput`]).
//!
//! The range that ends the block checks the header's gas used, bloom and
//! blob gas against its totals, which continue the witnessed start; the
//! aggregation of the outputs (see `aggregation`) links every start to the
//! previous range's end and repeats the checks over the chain.

use super::index_trie;
use super::io::GlamsterdamIO;
use super::metadata_op::GlamsterdamBlockMetadata;
use super::receipt::ReceiptReflection;
use super::requests::{
    witness_deposit_requests_hasher, RangeRequests, Sha256Accumulator, SystemRequests,
};
use super::storage_policy::GlamsterdamStorageAccessCostModel;
use super::witness::{
    BlockAccessIndexRange, BlockProgress, BlockProgressQuery, InitialValues, TransactionRange,
};
use crate::bootloader::block_flow::eip_2935_historical_block_hash::eip2935_system_part;
use crate::bootloader::block_flow::ethereum::block_header::PectraForkHeader;
use crate::bootloader::block_flow::ethereum::eip_4788_historical_beacon_root::eip4788_system_part;
use crate::bootloader::block_flow::ethereum::eip_7002_withdrawal_contract::eip7002_system_part;
use crate::bootloader::block_flow::ethereum::eip_7251_consolidation_contract::eip7251_system_part;
use crate::bootloader::block_flow::ethereum::oracle_queries::{
    ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID, ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID,
};
use crate::bootloader::block_flow::ethereum::withdrawals::{
    process_withdrawals_list, WithdrawalsList,
};
use crate::bootloader::block_flow::ethereum_glamsterdam::block_header::{
    verify_history, GlamsterdamHeader,
};
use crate::bootloader::block_flow::ethereum_glamsterdam::eip_8282_builder_requests::{
    eip8282_builder_deposits_system_part, eip8282_builder_exits_system_part,
};
use crate::bootloader::block_flow::ethereum_glamsterdam::index_mask::IndexMask;
use crate::bootloader::block_flow::ethereum_glamsterdam::GlamsterdamError;
use crate::bootloader::block_flow::tx_loop::TxLoopOp;
use crate::bootloader::block_flow::{PostTxLoopOp, PreTxLoopOp};
use crate::bootloader::errors::{BootloaderSubsystemError, TxError};
use crate::bootloader::result_keeper::ResultKeeperExt;
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::supported_ees::EthereumLikeSystem;
use crate::bootloader::transaction::TxBytes;
use crate::bootloader::transaction_flow::ethereum_glamsterdam::{
    GlamsterdamTransactionDataKeeper, GlamsterdamTransactionFlow,
};
use crate::bootloader::transaction_flow::logs_bloom::LogsBloom;
use crate::bootloader::transaction_flow::MinimalTransactionOutput;
use crate::bootloader::{BasicBootloader, BasicBootloaderExecutionConfig};
use alloc::boxed::Box;
use basic_system::system_implementation::ethereum_storage_model::vec_trait::VecLikeCtor;
use basic_system::system_implementation::ethereum_storage_model::{
    BoxInterner, MptPreimagesOracle, EMPTY_ROOT_HASH,
};
use core::alloc::Allocator;
use core::fmt::Write;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;

use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::basic_queries::DisconnectOracleQuery;
use zk_ee::oracle::memory_io::OracleQuery;
use zk_ee::oracle::IOOracle;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::validator::TxValidator;
use zk_ee::system::{
    AccountDataRequest, IOResultKeeper, IOSubsystemExt, IOTeardown, Resource, StateGasResources,
    System,
};
use zk_ee::system_log;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

/// Capacity of the interner that holds the transactions and receipts trie nodes the range
/// opens: the leaves are the transactions and receipts themselves
const TRIE_NODES_CAPACITY: usize = 1 << 24;

/// What the loop carries between the ops
pub struct GlamsterdamRangeData<A: Allocator + 'static> {
    pub keeper: GlamsterdamTransactionDataKeeper,
    /// The number of transactions of the block, as witnessed
    pub transaction_count: u32,
    /// The block's accounting at the start of the range, as witnessed
    pub start: BlockProgress,
    pub transactions_root: Bytes32,
    pub receipts_root: Bytes32,
    /// The nodes of the transactions and receipts tries the range opens; a transaction
    /// runs from its leaf in here, so the buffer lives for the whole run
    pub nodes: &'static mut BoxInterner<A>,
}

/// What a range establishes, for the aggregation of the ranges of a block
/// (see `aggregation`)
pub struct GlamsterdamRangeOutput<A: Allocator> {
    pub header_hash: Bytes32,
    /// The transaction range, as witnessed
    pub transactions: TransactionRange,
    /// The block access indices the range covered (from `transactions` and
    /// `transaction_count`)
    pub range: BlockAccessIndexRange,
    /// The number of transactions of the block, as witnessed, unverified: the
    /// aggregation requires the pre-execution check's
    pub transaction_count: u32,
    /// The block's accounting at the start of the range as witnessed, unverified
    /// (nothing for a range starting at the first transaction or before it): the
    /// aggregation links it to the previous range's end
    pub start: BlockProgress,
    /// The receipts' cumulative gas after the last transaction of the range
    pub cumulative_gas_used: u64,
    /// The block's execution and state gas (EIP-8037) and blob gas after the range
    pub block_execution_gas_used: u64,
    pub block_state_gas_used: u64,
    pub block_blob_gas_used: u64,
    /// The block's logs bloom after the range: the start's with the logs of
    /// the range's transactions marked
    pub bloom: LogsBloom,
    /// The header's gas used, for the linkage
    pub header_gas_used: u64,
    /// The deposit request hashing states the range started from and ended
    /// with and, for the range that ran the post-transaction phase, the
    /// system requests
    pub requests: RangeRequests,
    /// The initial value tables the range ran on, with their hashes as
    /// witnessed: the aggregation requires the pre-execution check's
    pub initial_values: InitialValues<A>,
    /// Which records of the access list the range accessed, reverted frames included:
    /// the accounts by address index, then the slots by position. The aggregation ORs
    /// the masks of the ranges and requires every bit set, so that an access list with
    /// entries nothing accessed is rejected.
    pub accessed: IndexMask<A>,
}

impl<A: Allocator> GlamsterdamRangeOutput<A> {
    /// The block's accounting after the range: what the next range starts from
    pub fn end(&self) -> BlockProgress {
        BlockProgress {
            cumulative_gas_used: self.cumulative_gas_used,
            execution_gas_used: self.block_execution_gas_used,
            state_gas_used: self.block_state_gas_used,
            blob_gas_used: self.block_blob_gas_used,
            logs_bloom: self.bloom,
        }
    }
}

impl<A: Allocator> core::fmt::Debug for GlamsterdamRangeOutput<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GlamsterdamRangeOutput")
            .field("header_hash", &self.header_hash)
            .field("transactions", &self.transactions)
            .field("range", &self.range)
            .field("transaction_count", &self.transaction_count)
            .field("start", &self.start)
            .field("cumulative_gas_used", &self.cumulative_gas_used)
            .field("block_execution_gas_used", &self.block_execution_gas_used)
            .field("block_state_gas_used", &self.block_state_gas_used)
            .field("block_blob_gas_used", &self.block_blob_gas_used)
            .field("deposit_requests", &self.requests.deposit_requests())
            .field("system_requests", &self.requests.system)
            .field("initial_values_hashes", &self.initial_values.hashes)
            .field("accessed", &self.accessed)
            .finish_non_exhaustive()
    }
}

pub struct GlamsterdamPreOp;
pub struct GlamsterdamLoopOp;
pub struct GlamsterdamPostOp<VC: VecLikeCtor>(core::marker::PhantomData<VC>);

fn glamsterdam_error(e: GlamsterdamError) -> BootloaderSubsystemError {
    // the error kind is kept out of the proof's error (a literal), the cause is for the host log
    let _ = e;
    internal_error!("glamsterdam range check failed").into()
}

/// The leaf of an index trie, which must be present; the path is checked
/// against the trie of `count` items (see `index_trie`)
fn leaf<'a, A: Allocator + 'static>(
    oracle: &mut impl IOOracle,
    nodes: &mut BoxInterner<A>,
    root: &Bytes32,
    index: u32,
    count: u32,
) -> Result<&'a [u8], BootloaderSubsystemError>
where
    BoxInterner<A>: 'a,
{
    let mut preimages = MptPreimagesOracle(oracle);
    let mut hasher = Keccak256::new();
    index_trie::lookup(root, index, count, &mut preimages, nodes, &mut hasher)
        .map_err(glamsterdam_error)?
        .ok_or_else(|| internal_error!("an index trie leaf of the range is missing").into())
}

impl<
        A: Allocator + Clone + Default + 'static,
        R: StateGasResources + Default,
        SF: StackFactory<N>,
        const N: usize,
        O: IOOracle,
        const PROOF_ENV: bool,
        S: EthereumLikeSystem<
            IO = GlamsterdamIO<A, R, GlamsterdamStorageAccessCostModel, SF, N, O, PROOF_ENV>,
            Metadata = GlamsterdamBlockMetadata,
            Allocator = A,
            Resources = R,
        >,
    > PreTxLoopOp<S> for GlamsterdamPreOp
where
    S::IO: IOSubsystemExt,
{
    type PreTxLoopResult = GlamsterdamRangeData<A>;

    fn pre_op(
        system: &mut System<S>,
        _system_functions: &mut HooksStorage<S, S::Allocator>,
        _memories: RunnerMemoryBuffers<'_>,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) -> Result<Self::PreTxLoopResult, BootloaderSubsystemError> {
        let allocator = system.get_allocator();
        let header: &GlamsterdamHeader = &system.metadata.block_level.header;
        let transactions_root = header.base.transactions_root;
        let receipts_root = header.base.receipts_root;
        let parent_beacon_block_root = header.base.parent_beacon_block_root;
        let block_gas_limit = header.base.gas_limit;

        // the access list the range runs on is the header's
        if system.io.storage.access_list_hash != header.block_access_list_hash {
            return Err(internal_error!("the block access list is not the header's").into());
        }
        // the count is the witnessed one; the IO derived the access index range from it
        let transaction_count = system.io.transaction_count();
        let transactions = system.io.transactions();
        let range = system.io.range();
        // checked when the IO witnessed it; checked again here, where the range runs
        transactions
            .check(transaction_count)
            .map_err(glamsterdam_error)?;
        if transactions.is_empty() && transaction_count != 0 {
            return Err(internal_error!("an empty transaction range inside the block").into());
        }
        if BlockAccessIndexRange::of(transactions, transaction_count).map_err(glamsterdam_error)?
            != range
        {
            return Err(internal_error!("the access index range is not the transactions'").into());
        }
        // a change beyond the last index would escape every range (the
        // pre-execution check rejects it too, against the proven count)
        if system.io.storage.max_access_index() > transaction_count + 1 {
            return Err(internal_error!("the access list changes state after the block").into());
        }

        let nodes: &'static mut BoxInterner<A> = Box::leak(Box::new_in(
            BoxInterner::with_capacity_in(TRIE_NODES_CAPACITY, allocator.clone()),
            allocator,
        ));

        // the block's accounting at the start of the range: nothing before the first
        // transaction, witnessed otherwise (unverified: the aggregation links it to the
        // previous range's end, and the receipts of the range's transactions, which
        // carry the cumulative gas, are checked against the receipts trie)
        let start = if transactions.start == 0 {
            BlockProgress::default()
        } else {
            BlockProgressQuery::get(system.io.oracle(), ())?
        };
        let keeper = GlamsterdamTransactionDataKeeper::new(
            block_gas_limit,
            start.cumulative_gas_used,
            start.execution_gas_used,
            start.state_gas_used,
            start.blob_gas_used,
            start.logs_bloom,
        );

        if transactions.includes_pre_transactions() {
            debug_assert_eq!(range.first, 0);
            system.io.begin_access_index(0)?;
            eip4788_system_part(system, &parent_beacon_block_root)?;
            eip2935_system_part(system)?;
            system.io.finish_access_index()?;
        }

        Ok(GlamsterdamRangeData {
            keeper,
            transaction_count,
            start,
            transactions_root,
            receipts_root,
            nodes,
        })
    }
}

impl<
        A: Allocator + Clone + Default + 'static,
        R: StateGasResources + Default,
        SF: StackFactory<N>,
        const N: usize,
        O: IOOracle,
        const PROOF_ENV: bool,
        S: EthereumLikeSystem<
            IO = GlamsterdamIO<A, R, GlamsterdamStorageAccessCostModel, SF, N, O, PROOF_ENV>,
            Metadata = GlamsterdamBlockMetadata,
            Allocator = A,
            Resources = R,
        >,
    > TxLoopOp<S> for GlamsterdamLoopOp
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    type BlockDataKeeper = GlamsterdamRangeData<A>;
    type BatchDataKeeper = ();

    fn loop_op<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        mut memories: RunnerMemoryBuffers<'a>,
        data: &mut Self::BlockDataKeeper,
        _batch_data: &mut Self::BatchDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig>,
        tracer: &mut impl Tracer<S>,
        validator: &mut impl TxValidator<S>,
    ) -> Result<(), BootloaderSubsystemError> {
        cycle_marker::start!("run_tx_loop");
        let transactions = system.io.transactions();
        let transactions_root = data.transactions_root;
        let count = data.transaction_count;
        for index in transactions.transactions() {
            system.io.begin_access_index(index + 1)?;

            let nodes = &mut *data.nodes;
            let next = system.try_begin_next_static_tx(
                |io| -> Result<Option<(u32, &'static [u8])>, BootloaderSubsystemError> {
                    let bytes = leaf(io.oracle(), nodes, &transactions_root, index, count)?;
                    Ok(Some((index, bytes)))
                },
            );
            let (tx_number, bytes) = match next {
                Some(Ok(next)) => next,
                Some(Err(e)) => return Err(e),
                None => return Err(internal_error!("the transaction leaf provider stopped").into()),
            };

            // The coinbase is warm from the start (every transaction touches it when it
            // pays the fee, so the access list carries it). The precompiles are not
            // warmed up: the storage model never charges them cold, and a warm-up would
            // mark a precompile the block never called as accessed, which the
            // aggregation's exactness check must not see.
            {
                let mut inf_resources =
                    <S::IO as zk_ee::system::IOSubsystem>::Resources::FORMAL_INFINITE;
                system
                    .io
                    .read_account_properties(
                        ExecutionEnvironmentType::NoEE,
                        &mut inf_resources,
                        &system.get_coinbase(),
                        AccountDataRequest::empty(),
                    )
                    .map_err(|_| internal_error!("must heat coinbase"))?;
            }

            system_log!(system, "====================================\n");
            system_log!(system, "TX execution begins for transaction {tx_number}\n");
            tracer.begin_tx(bytes);
            cycle_marker::start!("process_transaction");
            let tx_result =
                BasicBootloader::<S, GlamsterdamTransactionFlow<S>>::process_transaction::<Config>(
                    TxBytes::Static(bytes),
                    system,
                    system_functions,
                    memories.reborrow(),
                    index == 0,
                    &mut data.keeper,
                    tracer,
                    validator,
                );
            cycle_marker::end!("process_transaction");
            tracer.finish_tx();

            match tx_result {
                Err(TxError::Internal(err)) => {
                    system_log!(system, "Tx execution result: Internal error = {err:?}\n");
                    return Err(err);
                }
                Err(TxError::Validation(err)) => {
                    // a block with an invalid transaction is invalid
                    system_log!(system, "Tx execution result: Validation error = {err:?}\n");
                    return Err(internal_error!("invalid transaction in the block").into());
                }
                Ok(result) => {
                    let output = result.into_bookkeeper_output();
                    system_log!(system, "Tx execution result = {:?}\n", &output);
                    result_keeper.tx_processed(Ok(output));
                }
            }

            check_receipt(system, data, index)?;
            // the state changes of the transaction are the declared ones
            system.finish_valid_tx()?;
            system_log!(system, "TX execution ends for transaction {tx_number}\n");
        }
        cycle_marker::end!("run_tx_loop");
        Ok(())
    }
}

/// Checks the transaction's receipt, from the receipts trie, against what the transaction
/// produced: its status, the cumulative gas, the logs and their bloom
fn check_receipt<
    A: Allocator + Clone + Default + 'static,
    R: StateGasResources + Default,
    SF: StackFactory<N>,
    const N: usize,
    O: IOOracle,
    const PROOF_ENV: bool,
    S: EthereumLikeSystem<
        IO = GlamsterdamIO<A, R, GlamsterdamStorageAccessCostModel, SF, N, O, PROOF_ENV>,
        Metadata = GlamsterdamBlockMetadata,
        Allocator = A,
        Resources = R,
    >,
>(
    system: &mut System<S>,
    data: &mut GlamsterdamRangeData<A>,
    index: u32,
) -> Result<(), BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    let receipts_root = data.receipts_root;
    let encoded = leaf(
        system.io.oracle(),
        data.nodes,
        &receipts_root,
        index,
        data.transaction_count,
    )?;
    let receipt = ReceiptReflection::parse(encoded).map_err(glamsterdam_error)?;
    let Some((status, cumulative_gas_used, num_events, bloom)) = data.keeper.last_receipt.take()
    else {
        return Err(internal_error!("no receipt data recorded for the transaction").into());
    };
    let mismatch = |what: &'static str| -> BootloaderSubsystemError {
        let mut logger = system.get_logger();
        let _ = logger.write_fmt(format_args!("Receipt mismatch: {what}\n"));
        internal_error!("receipt mismatch").into()
    };
    if receipt.status != status {
        return Err(mismatch("status"));
    }
    if receipt.cumulative_gas_used != cumulative_gas_used {
        return Err(mismatch("cumulative gas used"));
    }
    if receipt.bloom != bloom.as_bytes() {
        return Err(mismatch("bloom"));
    }
    let mut events = system.io.events_in_this_tx_iterator();
    let mut logs = 0usize;
    for log in receipt.logs() {
        let log = log.map_err(glamsterdam_error)?;
        let Some(event) = events.next() else {
            return Err(mismatch("more logs in the receipt than emitted"));
        };
        if *event.address != log.address || event.data != log.data {
            return Err(mismatch("log address or data"));
        }
        let mut topics = event.topics.iter();
        for topic in log.topics() {
            let topic = topic.map_err(glamsterdam_error)?;
            if topics.next().map(|t| t.as_u8_array_ref()) != Some(topic) {
                return Err(mismatch("log topic"));
            }
        }
        if topics.next().is_some() {
            return Err(mismatch("log topics"));
        }
        logs += 1;
    }
    if events.next().is_some() || logs != num_events {
        return Err(mismatch("number of logs"));
    }
    Ok(())
}

impl<
        A: Allocator + Clone + Default + 'static,
        R: StateGasResources + Default,
        SF: StackFactory<N>,
        const N: usize,
        O: IOOracle,
        const PROOF_ENV: bool,
        VC: VecLikeCtor,
        S: EthereumLikeSystem<
            IO = GlamsterdamIO<A, R, GlamsterdamStorageAccessCostModel, SF, N, O, PROOF_ENV>,
            Metadata = GlamsterdamBlockMetadata,
            Allocator = A,
            Resources = R,
        >,
    > PostTxLoopOp<S> for GlamsterdamPostOp<VC>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    type BlockDataKeeper = GlamsterdamRangeData<A>;
    type BatchDataKeeper = ();
    type PostTxLoopOpResult = (O, GlamsterdamRangeOutput<A>);
    type BlockHeader = GlamsterdamHeader;

    fn post_op(
        mut system: System<S>,
        data: Self::BlockDataKeeper,
        _batch_data: &mut Self::BatchDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig, BlockHeader = Self::BlockHeader>,
    ) -> Result<Self::PostTxLoopOpResult, BootloaderSubsystemError> {
        let range = system.io.range();
        let transactions = system.io.transactions();
        let count = data.transaction_count;
        // the range ending the block checks the block's totals against the header; it
        // started from the witnessed accounting after the previous ranges, which the
        // aggregation links to their ends
        let ends_block = transactions.includes_post_transactions(count);

        // the deposit request hashing: from the head for the first range, from the
        // witnessed state after the previous ranges otherwise
        let hasher_start = if transactions.includes_pre_transactions() {
            Sha256Accumulator::deposit_requests_head()
        } else {
            witness_deposit_requests_hasher(system.io.oracle()).map_err(glamsterdam_error)?
        };

        let mut system_requests = None;
        if transactions.includes_post_transactions(count) {
            debug_assert_eq!(range.last, count + 1);
            system.io.begin_access_index(count + 1)?;
            let handle = system.start_global_frame()?;

            let withdrawals_root = {
                let withdrawals_encoding = system.get_bytes_from_query(
                    ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID,
                    ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID,
                )?;
                match withdrawals_encoding {
                    Some(withdrawals) => {
                        let withdrawals_list =
                            WithdrawalsList::decode_list_full(withdrawals.as_slice())
                                .map_err(|_| internal_error!("withdrawals list is invalid"))?;
                        if withdrawals_list.count.unwrap_or(0) > 0 {
                            process_withdrawals_list::<S, VC>(&mut system, withdrawals_list)?
                        } else {
                            EMPTY_ROOT_HASH
                        }
                    }
                    None => EMPTY_ROOT_HASH,
                }
            };
            if withdrawals_root != system.metadata.block_level.header.base.withdrawals_root {
                return Err(internal_error!("withdrawals root diverged").into());
            }

            // the system request types in order (the deposit requests, type 0,
            // are hashed from the logs below): the hash of every non-empty one
            use crypto::sha256::Digest;
            let mut requests = SystemRequests::default();
            let mut hasher = crypto::sha256::Sha256::new();
            let push = |requests: &mut SystemRequests,
                        request_type: u8,
                        hasher: &mut crypto::sha256::Sha256|
             -> Result<(), BootloaderSubsystemError> {
                requests
                    .push(
                        request_type,
                        Bytes32::from_array(hasher.finalize_reset().into()),
                    )
                    .map_err(glamsterdam_error)
            };
            // EIP-7002 withdrawal and EIP-7251 consolidation requests, types 1 and 2
            if eip7002_system_part(&mut system, &mut hasher)? {
                push(&mut requests, 1, &mut hasher)?;
            }
            if eip7251_system_part(&mut system, &mut hasher)? {
                push(&mut requests, 2, &mut hasher)?;
            }
            // EIP-8282: the builder deposit and exit requests, types 3 and 4
            if eip8282_builder_deposits_system_part(&mut system, &mut hasher)? {
                push(&mut requests, 3, &mut hasher)?;
            }
            if eip8282_builder_exits_system_part(&mut system, &mut hasher)? {
                push(&mut requests, 4, &mut hasher)?;
            }
            system_requests = Some(requests);

            system.finish_global_frame(None)?;
            drop(handle);
            system.io.finish_access_index()?;
        }

        // the deposit requests of the range's transactions, absorbed after they ran;
        // the range that ends the block finishes the requests hash
        let requests = RangeRequests::collect(&system, hasher_start, system_requests)
            .map_err(glamsterdam_error)?;
        if let Some(requests_hash) = requests.block_requests_hash() {
            if requests_hash != system.metadata.block_level.header.base.requests_hash {
                return Err(internal_error!("requests hash diverged").into());
            }
        }

        let header: &PectraForkHeader = &system.metadata.block_level.header.base;
        if ends_block {
            if data.keeper.block_gas_used() != header.gas_used {
                let mut logger = system.get_logger();
                let _ = logger.write_fmt(format_args!(
                    "Block gas used diverged: header {}, ours max({} execution, {} state), cumulative {}\n",
                    header.gas_used,
                    data.keeper.block_execution_gas_used,
                    data.keeper.block_state_gas_used,
                    data.keeper.cumulative_gas_used
                ));
                return Err(internal_error!("gas used diverged").into());
            }
            if data.keeper.block_bloom != header.logs_bloom {
                return Err(internal_error!("block bloom diverged").into());
            }
            if data.keeper.block_blob_gas_used != header.blob_gas_used {
                return Err(internal_error!("blob gas used diverged").into());
            }
        }

        // the block hashes the range read through BLOCKHASH follow from the header chain
        {
            let allocator = system.get_allocator();
            let block_level = &system.metadata.block_level;
            // SAFETY: the cache is read here only, after the transactions ran
            let cache = unsafe { block_level.history_cache.as_ref_unchecked() };
            verify_history(&block_level.header, cache, &mut system.io.oracle, allocator)
                .map_err(glamsterdam_error)?;
        }

        let accessed = system
            .io
            .storage
            .accessed_mask()
            .map_err(glamsterdam_error)?;
        let header_hash = system.metadata.block_level.computed_header_hash;
        let header_gas_used = header.gas_used;

        let System {
            mut io, metadata, ..
        } = system;
        result_keeper.record_sealed_block(metadata.block_level.header);
        io.report_new_preimages(result_keeper);
        result_keeper.events(io.events_iterator());
        <DisconnectOracleQuery as OracleQuery>::get(&mut io.oracle, ())?;
        let GlamsterdamIO {
            storage, oracle, ..
        } = io;

        let output = GlamsterdamRangeOutput {
            header_hash,
            transactions,
            range,
            transaction_count: count,
            start: data.start,
            cumulative_gas_used: data.keeper.cumulative_gas_used,
            block_execution_gas_used: data.keeper.block_execution_gas_used,
            block_state_gas_used: data.keeper.block_state_gas_used,
            block_blob_gas_used: data.keeper.block_blob_gas_used,
            bloom: data.keeper.block_bloom,
            header_gas_used,
            requests,
            initial_values: storage.initial,
            accessed,
        };
        Ok((oracle, output))
    }
}

/// Keeps the unused generic parameter honest
impl<VC: VecLikeCtor> Default for GlamsterdamPostOp<VC> {
    fn default() -> Self {
        Self(core::marker::PhantomData)
    }
}

// the data the loop keeps is only ever used by this module's ops
impl<A: Allocator + 'static> core::fmt::Debug for GlamsterdamRangeData<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GlamsterdamRangeData")
            .field("keeper", &self.keeper)
            .field("transaction_count", &self.transaction_count)
            .finish()
    }
}
