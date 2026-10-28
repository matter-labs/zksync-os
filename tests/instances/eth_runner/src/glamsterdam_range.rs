//! Host side of the Glamsterdam transaction-range validation: the
//! transactions and receipts tries of a fixture, the responder serving the
//! transaction range, the transaction count and the initial value tables of
//! the pre-execution check, the oracle assembling all of it, the split of a
//! block into subranges by gas and the aggregation of their outputs.

use crate::glamsterdam::GlamsterdamBlock;
use crate::glamsterdam_state::{
    access_list_encoding, make_state_oracle_from_parts, run_state_consistency,
};
use alloy::consensus::Header;
use alloy_rlp::Decodable;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::block_header::GlamsterdamHeader;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::linkage::{
    LinkageAggregation, ProgramOutput,
};
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::state_consistency::StateConsistencyOutput;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::aggregation::{
    PreExecutionSummary, RangeAggregation,
};
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::block_flow::GlamsterdamRangeOutput;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::oracle_queries::*;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::receipt::ReceiptReflection;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::requests::Sha256Accumulator;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::witness::{
    BlockProgress, TransactionRange,
};
use basic_bootloader::bootloader::config::BasicBootloaderForwardETHLikeConfig;
use basic_bootloader::bootloader::transaction::TxEncodingFormat;
use basic_bootloader::bootloader::transaction_flow::ethereum_glamsterdam::GlamsterdamTransactionFlow;
use basic_bootloader::bootloader::BasicBootloader;
use basic_system::system_implementation::ethereum_storage_model::reference_trie::{
    build_trie_with_paths, index_key_nibbles, rlp_string,
};
use oracle_provider::{
    respond_to_every_target, OracleQueryProcessor, ReadWitnessSource, RunMode,
    ZkEENonDeterminismSource,
};
use rig::forward_system::run::result_keeper::ForwardRunningResultKeeper;
use rig::forward_system::run::test_impl::NoopTxCallback;
use rig::forward_system::system::system_types::glamsterdam::GlamsterdamSystemTypes;
use ruint::aliases::B160;
use std::alloc::Global;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use zk_ee::oracle::memory_io::host::{write_dynamic_bytes, QuerierMemory, WriteQueryOutput};
use zk_ee::oracle::query_ids::{TX_ENCODING_FORMAT_QUERY_ID, TX_FROM_QUERY_ID};
use zk_ee::system::metadata::chain_config::ChainConfig;
use zk_ee::system::tracer::NopTracer;
use zk_ee::system::validator::NopTxValidator;
use zk_ee::utils::Bytes32;

/// The leaf value item of a transaction or receipt in its trie: the encoded
/// transaction or receipt (a typed envelope or a legacy RLP list) as a string
fn leaf_item(encoded: &[u8]) -> Vec<u8> {
    rlp_string(encoded)
}

/// Builds an index trie over `items`; returns the root and its nodes by hash
pub fn index_trie(items: &[Vec<u8>]) -> ([u8; 32], BTreeMap<Bytes32, Vec<u8>>) {
    let mut store = BTreeMap::new();
    let entries = items
        .iter()
        .enumerate()
        .map(|(index, item)| (index_key_nibbles(index as u32), leaf_item(item)))
        .collect();
    let root = build_trie_with_paths(entries, &mut store);
    (root, store)
}

/// The transactions and receipts tries of a fixture, checked against its header
pub struct IndexTries {
    pub transactions: Vec<Vec<u8>>,
    pub receipts: Vec<Vec<u8>>,
    pub nodes: BTreeMap<Bytes32, Vec<u8>>,
}

impl IndexTries {
    pub fn of(fixture: &GlamsterdamBlock) -> anyhow::Result<Self> {
        let body = crate::glamsterdam::RawBlockBody::decode(&fixture.raw_block)?;
        let transactions: Vec<Vec<u8>> = body.transactions.iter().map(|tx| tx.to_vec()).collect();
        let receipts: Vec<Vec<u8>> = fixture.raw_receipts.iter().map(|r| r.to_vec()).collect();
        let header = &fixture.block.header.inner;
        let (transactions_root, mut nodes) = index_trie(&transactions);
        anyhow::ensure!(
            transactions_root == header.transactions_root.0,
            "the transactions trie does not match the header"
        );
        let (receipts_root, receipt_nodes) = index_trie(&receipts);
        anyhow::ensure!(
            receipts_root == header.receipts_root.0,
            "the receipts trie does not match the header"
        );
        nodes.extend(receipt_nodes);
        Ok(Self {
            transactions,
            receipts,
            nodes,
        })
    }
}

/// The initial value tables a range witnesses, as the pre-execution check
/// produces them (`StateConsistencyOutput::initial_value_tables`): kept apart
/// from the pre-execution output so that the ranges of a block run from one
/// native pre-execution run, or from tables stored by it, without replaying it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitialValueTables {
    pub accounts: Vec<u8>,
    pub fresh_accounts: Vec<u8>,
    pub slots: Vec<u8>,
}

impl InitialValueTables {
    pub fn of(state: &StateConsistencyOutput<Global>) -> Self {
        let [accounts, fresh_accounts, slots] = state.initial_value_tables().map(<[u8]>::to_vec);
        Self {
            accounts,
            fresh_accounts,
            slots,
        }
    }

    /// Stores the tables: three `length (u32 little-endian) || bytes` entries
    pub fn write(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let mut bytes = Vec::new();
        for table in [&self.accounts, &self.fresh_accounts, &self.slots] {
            bytes.extend_from_slice(&(table.len() as u32).to_le_bytes());
            bytes.extend_from_slice(table);
        }
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

/// Serves the transaction range and the initial value tables (the transaction
/// count comes from the shared `TransactionCountResponder`)
#[derive(Clone, Debug)]
pub struct GlamsterdamRangeResponder {
    pub range: TransactionRange,
    pub tables: InitialValueTables,
}

impl GlamsterdamRangeResponder {
    pub fn new(range: TransactionRange, tables: &InitialValueTables) -> Self {
        Self {
            range,
            tables: tables.clone(),
        }
    }
}

impl OracleQueryProcessor for GlamsterdamRangeResponder {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        vec![
            GLAMSTERDAM_TRANSACTION_RANGE_QUERY_ID,
            GLAMSTERDAM_INITIAL_ACCOUNTS_LEN_QUERY_ID,
            GLAMSTERDAM_INITIAL_ACCOUNTS_DATA_QUERY_ID,
            GLAMSTERDAM_FRESH_ACCOUNTS_LEN_QUERY_ID,
            GLAMSTERDAM_FRESH_ACCOUNTS_DATA_QUERY_ID,
            GLAMSTERDAM_INITIAL_SLOTS_LEN_QUERY_ID,
            GLAMSTERDAM_INITIAL_SLOTS_DATA_QUERY_ID,
        ]
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        _input_word: usize,
        _memory: &dyn QuerierMemory,
        mode: RunMode,
        native_run_responses: &mut Vec<u32>,
        guest_run_responses: &mut Vec<u32>,
    ) {
        let mut response = Vec::new();
        let len = |buffer: &Vec<u8>, response: &mut Vec<u32>| {
            (buffer.len() as u32).write_output(response)
        };
        match query_id {
            GLAMSTERDAM_TRANSACTION_RANGE_QUERY_ID => self.range.write_output(&mut response),
            GLAMSTERDAM_INITIAL_ACCOUNTS_LEN_QUERY_ID => len(&self.tables.accounts, &mut response),
            GLAMSTERDAM_INITIAL_ACCOUNTS_DATA_QUERY_ID => {
                write_dynamic_bytes(&self.tables.accounts, &mut response)
            }
            GLAMSTERDAM_FRESH_ACCOUNTS_LEN_QUERY_ID => {
                len(&self.tables.fresh_accounts, &mut response)
            }
            GLAMSTERDAM_FRESH_ACCOUNTS_DATA_QUERY_ID => {
                write_dynamic_bytes(&self.tables.fresh_accounts, &mut response)
            }
            GLAMSTERDAM_INITIAL_SLOTS_LEN_QUERY_ID => len(&self.tables.slots, &mut response),
            GLAMSTERDAM_INITIAL_SLOTS_DATA_QUERY_ID => {
                write_dynamic_bytes(&self.tables.slots, &mut response)
            }
            _ => unreachable!("not a range query: 0x{query_id:08x}"),
        }
        respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
    }
}

/// What the ranges before a range established, which the range witnesses
/// unverified and the aggregation links: the block's accounting (the
/// receipts' cumulative gas, the gas sums, the logs bloom) and the sha256
/// state of the deposit request hashing (EIP-6110) after their transactions
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeStart {
    pub progress: BlockProgress,
    pub hasher: Sha256Accumulator,
}

impl Default for RangeStart {
    /// The start of the first range
    fn default() -> Self {
        Self {
            progress: BlockProgress::default(),
            hasher: Sha256Accumulator::deposit_requests_head(),
        }
    }
}

impl RangeStart {
    /// The start of the range after `outputs`, the outputs of the ranges so far
    pub fn after(outputs: &[GlamsterdamRangeOutput<Global>]) -> Self {
        match outputs.last() {
            Some(previous) => Self {
                progress: previous.end(),
                hasher: previous.requests.end,
            },
            None => Self::default(),
        }
    }
}

/// Serves what the transactions of a range need from the host besides their
/// bytes (which come from the transactions trie): the encoding format and the
/// sender of every transaction in order, and what the ranges before it
/// established
#[derive(Clone, Debug)]
pub struct GlamsterdamTxResponder {
    pub senders: VecDeque<B160>,
    pub start: RangeStart,
}

impl OracleQueryProcessor for GlamsterdamTxResponder {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        vec![
            TX_ENCODING_FORMAT_QUERY_ID,
            TX_FROM_QUERY_ID,
            GLAMSTERDAM_BLOCK_PROGRESS_QUERY_ID,
            GLAMSTERDAM_DEPOSIT_REQUESTS_HASHER_QUERY_ID,
        ]
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        _input_word: usize,
        _memory: &dyn QuerierMemory,
        mode: RunMode,
        native_run_responses: &mut Vec<u32>,
        guest_run_responses: &mut Vec<u32>,
    ) {
        let mut response = Vec::new();
        match query_id {
            TX_ENCODING_FORMAT_QUERY_ID => TxEncodingFormat::Rlp.write_output(&mut response),
            TX_FROM_QUERY_ID => self
                .senders
                .pop_front()
                .expect("a sender for every transaction of the range")
                .write_output(&mut response),
            GLAMSTERDAM_BLOCK_PROGRESS_QUERY_ID => self.start.progress.write_output(&mut response),
            GLAMSTERDAM_DEPOSIT_REQUESTS_HASHER_QUERY_ID => {
                self.start.hasher.write_output(&mut response)
            }
            _ => unreachable!("not a transaction query: 0x{query_id:08x}"),
        }
        respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
    }
}

/// The senders of the transactions of the range, in order
fn range_senders(fixture: &GlamsterdamBlock, range: TransactionRange) -> VecDeque<B160> {
    let transactions = fixture.block.transactions.clone().into_transactions_vec();
    range
        .transactions()
        .map(|index| {
            let signer = transactions[index as usize].inner.signer();
            B160::try_from_be_slice(signer.as_slice()).expect("an address")
        })
        .collect()
}

/// The oracle of a transaction range of a fixture: everything the
/// pre-execution check needs, the transactions and receipts tries, the
/// range responder serving the initial value tables of the pre-execution check,
/// and the transactions' senders. `start` is what the ranges before it
/// established.
pub fn make_range_oracle(
    fixture: &GlamsterdamBlock,
    tries: &IndexTries,
    tables: &InitialValueTables,
    range: TransactionRange,
    start: &RangeStart,
    mode: RunMode,
) -> anyhow::Result<ZkEENonDeterminismSource> {
    let target_header = Header::decode(&mut fixture.raw_header.as_ref())?;
    let parents: Vec<(Header, Vec<u8>)> = fixture
        .witness
        .headers
        .iter()
        .map(|raw| Ok((Header::decode(&mut raw.as_ref())?, raw.to_vec())))
        .collect::<Result<_, alloy_rlp::Error>>()?;
    let preimages = fixture
        .witness
        .state
        .iter()
        .chain(fixture.witness.codes.iter())
        .map(|node| node.to_vec())
        .chain(tries.nodes.values().cloned());
    // the body's withdrawals, as the node encodes them
    let body = crate::glamsterdam::RawBlockBody::decode(&fixture.raw_block)?;
    let withdrawals = body.trailing.get(1).map(|w| w.to_vec()).unwrap_or_default();
    // the count responder of the shared oracle is the one the range reads too
    let mut oracle = make_state_oracle_from_parts(
        (target_header, fixture.raw_header.to_vec()),
        parents,
        preimages,
        access_list_encoding(fixture),
        withdrawals,
        tries.transactions.len() as u32,
        mode,
    );
    oracle.add_external_processor(GlamsterdamRangeResponder::new(range, tables));
    oracle.add_external_processor(GlamsterdamTxResponder {
        senders: range_senders(fixture, range),
        start: start.clone(),
    });
    // the advice oracles of the proving system functions (signature recovery, big-integer
    // arithmetic, blob commitments), as the rig's Ethereum block oracle registers them
    if mode.produces_native_run_responses() {
        oracle.add_external_processor(
            rig::callable_oracles::blob_kzg_commitment::NativeBlobCommitmentAndProofQuery,
        );
        oracle.add_external_processor(rig::callable_oracles::arithmetic::NativeArithmeticQuery);
        oracle.add_external_processor(rig::callable_oracles::field_hints::NativeFieldOpsQuery);
    } else {
        oracle.add_external_processor(
            rig::callable_oracles::blob_kzg_commitment::BlobCommitmentAndProofQuery,
        );
        oracle.add_external_processor(rig::callable_oracles::arithmetic::ArithmeticQuery);
        oracle.add_external_processor(rig::callable_oracles::field_hints::FieldOpsQuery);
    }
    Ok(oracle)
}

/// The system types of a host run of a range
pub type HostRangeSystem = GlamsterdamSystemTypes<ZkEENonDeterminismSource, false>;

/// The system types of a host run of a range that records the words the
/// RISC-V guest reads: with the advice-based system functions of the guest
/// (`PROOF_ENV`), so that the hint queries and their answers are recorded too
pub type RecordingRangeSystem = GlamsterdamSystemTypes<ReadWitnessSource, true>;

/// Runs a range of a fixture on the host and returns what it established
pub fn run_range(
    fixture: &GlamsterdamBlock,
    tries: &IndexTries,
    tables: &InitialValueTables,
    range: TransactionRange,
    start: &RangeStart,
) -> anyhow::Result<GlamsterdamRangeOutput<Global>> {
    run_range_with_tracer(
        fixture,
        tries,
        tables,
        range,
        start,
        &mut NopTracer::default(),
    )
}

/// `run_range` with a tracer
pub fn run_range_with_tracer(
    fixture: &GlamsterdamBlock,
    tries: &IndexTries,
    tables: &InitialValueTables,
    range: TransactionRange,
    start: &RangeStart,
    tracer: &mut impl zk_ee::system::tracer::Tracer<HostRangeSystem>,
) -> anyhow::Result<GlamsterdamRangeOutput<Global>> {
    let oracle = make_range_oracle(fixture, tries, tables, range, start, RunMode::NativeRunOnly)?;
    let mut result_keeper =
        ForwardRunningResultKeeper::<NoopTxCallback, GlamsterdamHeader>::new(NoopTxCallback);
    let (_oracle, output) = BasicBootloader::<
        HostRangeSystem,
        GlamsterdamTransactionFlow<HostRangeSystem>,
    >::run_prepared::<BasicBootloaderForwardETHLikeConfig>(
        oracle,
        &mut (),
        &mut result_keeper,
        tracer,
        &mut NopTxValidator,
        ChainConfig::default(),
    )
    .map_err(|e| anyhow::anyhow!("range run failed: {e:?}"))?;
    Ok(output)
}

/// Runs a range on the host while recording the oracle's responses as the
/// RISC-V guest reads them: the words the `glamsterdam_tx_range` program runs
/// from in the simulator (`QuasiUARTSource::new_with_reads`)
pub fn run_range_recording(
    fixture: &GlamsterdamBlock,
    tries: &IndexTries,
    tables: &InitialValueTables,
    range: TransactionRange,
    start: &RangeStart,
) -> anyhow::Result<(GlamsterdamRangeOutput<Global>, Vec<u32>)> {
    let oracle = make_range_oracle(
        fixture,
        tries,
        tables,
        range,
        start,
        RunMode::NativeRunSavingForRiscV,
    )?;
    let source = ReadWitnessSource::new(oracle);
    let mut result_keeper =
        ForwardRunningResultKeeper::<NoopTxCallback, GlamsterdamHeader>::new(NoopTxCallback);
    let (source, output) = BasicBootloader::<
        RecordingRangeSystem,
        GlamsterdamTransactionFlow<RecordingRangeSystem>,
    >::run_prepared::<BasicBootloaderForwardETHLikeConfig>(
        source,
        &mut (),
        &mut result_keeper,
        &mut NopTracer::default(),
        &mut NopTxValidator,
        ChainConfig::default(),
    )
    .map_err(|e| anyhow::anyhow!("range run failed: {e:?}"))?;
    let words = source.get_read_items().borrow().clone();
    Ok((output, words))
}

/// One EVM step of a traced transaction
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub depth: u32,
    pub pc: usize,
    pub opcode: u8,
    pub gas: u64,
}

/// Records the EVM steps of one transaction of a range (by its number in the run), as
/// the node's `debug_traceTransaction` struct logs do, for a diff against them
#[derive(Default)]
pub struct StepTracer {
    pub target_tx: u32,
    pub current_tx: Option<u32>,
    pub depth: u32,
    pub steps: Vec<Step>,
}

impl StepTracer {
    pub fn new(target_tx: u32) -> Self {
        Self {
            target_tx,
            ..Default::default()
        }
    }
    fn recording(&self) -> bool {
        self.current_tx == Some(self.target_tx)
    }
}

impl zk_ee::system::tracer::evm_tracer::EvmTracer<HostRangeSystem> for StepTracer {
    fn before_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl zk_ee::system::evm::EvmFrameInterface<HostRangeSystem>,
    ) {
        if self.recording() {
            use zk_ee::system::Resources;
            self.steps.push(Step {
                depth: self.depth,
                pc: frame_state.instruction_pointer(),
                opcode,
                gas: frame_state.resources().legacy_gas(),
            });
        }
    }
    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl zk_ee::system::evm::EvmFrameInterface<HostRangeSystem>,
    ) {
    }
    fn on_opcode_error(
        &mut self,
        _error: &zk_ee::system::evm::EvmError,
        _frame_state: &impl zk_ee::system::evm::EvmFrameInterface<HostRangeSystem>,
    ) {
    }
    fn on_call_error(&mut self, _error: &zk_ee::system::evm::EvmError) {}
    fn on_selfdestruct(
        &mut self,
        _beneficiary: B160,
        _token_value: ruint::aliases::U256,
        _frame_state: &impl zk_ee::system::evm::EvmFrameInterface<HostRangeSystem>,
    ) {
    }
    fn on_create_request(&mut self, _is_create2: bool) {}
}

impl zk_ee::system::tracer::Tracer<HostRangeSystem> for StepTracer {
    fn evm_tracer(
        &mut self,
    ) -> &mut impl zk_ee::system::tracer::evm_tracer::EvmTracer<HostRangeSystem> {
        self
    }
    fn on_new_execution_frame(
        &mut self,
        _request: &zk_ee::system::ExecutionEnvironmentLaunchParams<HostRangeSystem>,
    ) {
        self.depth += 1;
    }
    fn after_execution_frame_completed(
        &mut self,
        _result: Option<(
            &<HostRangeSystem as zk_ee::system::SystemTypes>::Resources,
            &zk_ee::system::CallResult<HostRangeSystem>,
        )>,
    ) {
        self.depth -= 1;
    }
    fn on_storage_read(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _is_transient: bool,
        _address: B160,
        _key: Bytes32,
        _value: Bytes32,
    ) {
    }
    fn on_storage_write(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _is_transient: bool,
        _address: B160,
        _key: Bytes32,
        _value: Bytes32,
    ) {
    }
    fn on_bytecode_change(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _address: B160,
        _new_raw_bytecode: Option<&[u8]>,
        _new_internal_bytecode_hash: Bytes32,
        _new_observable_bytecode_length: u32,
    ) {
    }
    fn on_event(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _address: &B160,
        _topics: &[Bytes32],
        _data: &[u8],
    ) {
    }
    fn begin_tx(&mut self, _calldata: &[u8]) {
        self.current_tx = Some(self.current_tx.map_or(0, |n| n + 1));
        self.depth = 0;
    }
    fn finish_tx(&mut self) {}
}

/// Diffs our steps of transaction `tx` of a whole-block run against the node's struct
/// logs (`debug_traceTransaction` with stack, memory and storage disabled): reports the
/// first step whose depth, program counter or gas differs
pub fn trace_diff(
    fixture: &GlamsterdamBlock,
    tries: &IndexTries,
    tx: u32,
    node_trace: &std::path::Path,
) -> anyhow::Result<()> {
    #[derive(serde::Deserialize)]
    struct Log {
        pc: usize,
        op: String,
        gas: u64,
        depth: u32,
    }
    #[derive(serde::Deserialize)]
    struct Trace {
        #[serde(rename = "structLogs")]
        struct_logs: Vec<Log>,
    }
    #[derive(serde::Deserialize)]
    struct Response {
        result: Trace,
    }
    let node: Response = serde_json::from_reader(std::fs::File::open(node_trace)?)?;
    let node = node.result.struct_logs;
    let mut tracer = StepTracer::new(tx);
    let tables = InitialValueTables::of(&run_state_consistency(fixture)?);
    let result = run_range_with_tracer(
        fixture,
        tries,
        &tables,
        whole_block(tries),
        &RangeStart::default(),
        &mut tracer,
    );
    println!(
        "run result: {}; our steps {}, node steps {}",
        result
            .as_ref()
            .map(|_| "ok".to_string())
            .unwrap_or_else(|e| e.to_string()),
        tracer.steps.len(),
        node.len()
    );
    let name =
        |opcode: u8| rig::evm_interpreter::opcodes::OPCODE_JUMPMAP[opcode as usize].unwrap_or("?");
    if result.is_err() {
        // the steps that led to the failure, whatever the node did
        let from = tracer.steps.len().saturating_sub(16);
        println!("our last steps before the failure:");
        for (j, step) in tracer.steps.iter().enumerate().skip(from) {
            println!(
                "  {j}: depth {} pc {} {} gas {}",
                step.depth,
                step.pc,
                name(step.opcode),
                step.gas
            );
        }
    }
    for (i, (ours, theirs)) in tracer.steps.iter().zip(node.iter()).enumerate() {
        // the node's depth is 1-based
        if ours.depth != theirs.depth || ours.pc != theirs.pc || ours.gas != theirs.gas {
            println!("first divergence at step {i}:");
            let window = i.saturating_sub(4)..(i + 2).min(tracer.steps.len().min(node.len()));
            for (j, (o, t)) in tracer
                .steps
                .iter()
                .zip(node.iter())
                .enumerate()
                .skip(window.start)
                .take(window.len())
            {
                println!(
                    "  {j}: ours depth {} pc {} {} gas {} | node depth {} pc {} {} gas {}",
                    o.depth,
                    o.pc,
                    name(o.opcode),
                    o.gas,
                    t.depth,
                    t.pc,
                    t.op,
                    t.gas
                );
            }
            return Ok(());
        }
    }
    println!("no divergence over the common prefix");
    Ok(())
}

/// The whole block as one range
pub fn whole_block(tries: &IndexTries) -> TransactionRange {
    TransactionRange {
        start: 0,
        end: tries.transactions.len() as u32,
    }
}

/// Splits the block into up to `parts` transaction ranges of roughly equal
/// cumulative gas (from the receipts); the first runs the pre-transaction
/// phase, the last the post-transaction phase
pub fn split_by_gas(tries: &IndexTries, parts: u32) -> anyhow::Result<Vec<TransactionRange>> {
    let count = tries.transactions.len() as u32;
    let cumulative = tries
        .receipts
        .iter()
        .map(|receipt| {
            ReceiptReflection::parse(receipt)
                .map(|receipt| receipt.cumulative_gas_used)
                .map_err(|e| anyhow::anyhow!("receipt: {e:?}"))
        })
        .collect::<anyhow::Result<Vec<u64>>>()?;
    let total = cumulative.last().copied().unwrap_or(0);
    let parts = parts.max(1) as u64;
    let mut ranges = Vec::new();
    let mut start = 0u32;
    for part in 1..parts {
        let target = total * part / parts;
        // the range ends after the transaction that reaches the target
        let end = cumulative
            .iter()
            .position(|gas| *gas >= target)
            .map(|index| index as u32 + 1)
            .unwrap_or(count);
        // a range holds a transaction, and the last range holds the last one
        if end <= start || end >= count {
            continue;
        }
        ranges.push(TransactionRange { start, end });
        start = end;
    }
    ranges.push(TransactionRange { start, end: count });
    Ok(ranges)
}

/// Runs the ranges of a block in order on the host, each from the gas the
/// previous one ended with
pub fn run_ranges(
    fixture: &GlamsterdamBlock,
    tries: &IndexTries,
    tables: &InitialValueTables,
    ranges: &[TransactionRange],
) -> anyhow::Result<Vec<GlamsterdamRangeOutput<Global>>> {
    let mut outputs: Vec<GlamsterdamRangeOutput<Global>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        let start = RangeStart::after(&outputs);
        let output = run_range(fixture, tries, tables, *range, &start)
            .map_err(|e| anyhow::anyhow!("range {}..{}: {e}", range.start, range.end))?;
        outputs.push(output);
    }
    Ok(outputs)
}

/// Aggregates the outputs of the ranges of a block against the pre-execution run:
/// the full host-side checks (`RangeAggregation`) and the aggregation program's
/// checks over the linkage outputs (`LinkageAggregation`), as the proofs would
/// expose them
pub fn aggregate(
    state: &StateConsistencyOutput<Global>,
    outputs: &[GlamsterdamRangeOutput<Global>],
) -> anyhow::Result<()> {
    let mut aggregation = RangeAggregation::new(PreExecutionSummary::of(state));
    for output in outputs {
        aggregation.add(output).map_err(|e| {
            anyhow::anyhow!(
                "range {}..{} does not aggregate: {e:?}",
                output.transactions.start,
                output.transactions.end
            )
        })?;
    }
    aggregation.finish().map_err(|e| {
        let (accounts, slots) = never_accessed(state, outputs);
        let hex = |address: &B160| format!("0x{:040x}", address.as_uint());
        anyhow::anyhow!(
            "the ranges do not aggregate to the block: {e:?}; never accessed: {} accounts {:?}, {} slots {:?}",
            accounts.len(),
            accounts.iter().take(8).map(hex).collect::<Vec<_>>(),
            slots.len(),
            slots
                .iter()
                .take(8)
                .map(|(address, key)| (hex(address), *key))
                .collect::<Vec<_>>()
        )
    })?;

    // the linkage: what the programs publish and the aggregation program checks
    let pre_execution = ProgramOutput::of_pre_execution(state);
    let mut linkage = LinkageAggregation::new(pre_execution.hash(), &pre_execution, Global)
        .map_err(|e| anyhow::anyhow!("linkage: {e:?}"))?;
    for output in outputs {
        let range = ProgramOutput::of_range(output);
        linkage
            .add(range.hash(), &range, &output.accessed)
            .map_err(|e| anyhow::anyhow!("linkage of range {:?}: {e:?}", output.transactions))?;
    }
    linkage
        .finish()
        .map_err(|e| anyhow::anyhow!("linkage: {e:?}"))
}

/// The accounts and the `(address, key)` slots of the access list no range accessed
pub fn never_accessed(
    state: &StateConsistencyOutput<Global>,
    outputs: &[GlamsterdamRangeOutput<Global>],
) -> (Vec<B160>, Vec<(B160, Bytes32)>) {
    let Some(first) = outputs.first() else {
        return (Vec::new(), Vec::new());
    };
    let mut accessed = first.accessed.clone();
    for output in &outputs[1..] {
        accessed.or_assign(&output.accessed).expect("one block");
    }
    let num_accounts = state.counts.accounts as u32;
    let address = |index: u32| state.addresses.key(index).0;
    let (accounts, slots): (Vec<u32>, Vec<u32>) = accessed
        .clear_indices()
        .partition(|index| *index < num_accounts);
    (
        accounts.into_iter().map(address).collect(),
        slots
            .into_iter()
            .map(|index| {
                let slot = &state.slots[(index - num_accounts) as usize];
                (
                    address(slot.address_index),
                    state.slot_keys.key(slot.key_index).0,
                )
            })
            .collect(),
    )
}

/// One line per range run
pub fn report(fixture: &GlamsterdamBlock, output: &GlamsterdamRangeOutput<Global>) -> String {
    format!(
        "block {}: transactions {}..{} of {} (access indices {}..={}) OK; cumulative gas {}, execution gas {}, state gas {}, blob gas {}; header gas used {}; accessed {}/{} access list records; {} deposit request bytes{}",
        fixture.block.header.inner.number,
        output.transactions.start,
        output.transactions.end,
        output.transaction_count,
        output.range.first,
        output.range.last,
        output.cumulative_gas_used,
        output.block_execution_gas_used,
        output.block_state_gas_used,
        output.block_blob_gas_used,
        fixture.block.header.inner.gas_used,
        output.accessed.count_ones(),
        output.accessed.len(),
        output.requests.deposit_requests(),
        match &output.requests.system {
            Some(system) => format!(
                ", requests hash checked with {} system request type(s)",
                system.types().len()
            ),
            None => String::new(),
        },
    )
}

/// Runs every fixture under `path` split into `parts` ranges by gas (one: the
/// whole block) and aggregates the outputs
pub fn check_path(path: &std::path::Path, parts: u32) -> anyhow::Result<usize> {
    let dirs = crate::glamsterdam::fixture_dirs(path)?;
    for dir in &dirs {
        let fixture = GlamsterdamBlock::from_dir(dir)?;
        let tries = IndexTries::of(&fixture)?;
        let state = run_state_consistency(&fixture)?;
        let tables = InitialValueTables::of(&state);
        let ranges = split_by_gas(&tries, parts)?;
        let outputs = run_ranges(&fixture, &tries, &tables, &ranges)
            .map_err(|e| anyhow::anyhow!("fixture {}: {e}", dir.display()))?;
        for output in &outputs {
            println!("{}", report(&fixture, output));
        }
        aggregate(&state, &outputs)
            .map_err(|e| anyhow::anyhow!("fixture {}: {e}", dir.display()))?;
        println!(
            "block {}: {} range(s) aggregate to the block",
            fixture.block.header.inner.number,
            outputs.len()
        );
    }
    Ok(dirs.len())
}

/// Runs every fixture under `path` split into `parts` ranges by gas on the RISC-V
/// simulator with the `app` program (a dist directory, see `load_program_for`), after
/// running and aggregating the ranges on the host, checks that every program returns
/// the hash of the range's linkage output, and prints the cycles per range and per block
pub fn run_on_simulator(path: &std::path::Path, app: &str, parts: u32) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let dirs = crate::glamsterdam::fixture_dirs(path)?;
    let program = crate::ethproofs::load_program_for(app)?;
    let mut total = 0u64;
    let mut total_gas = 0u64;
    println!(
        "{:>10} {:>10} {:>5} {:>9} {:>14} {:>10}",
        "block", "gas", "txs", "range", "cycles", "cycles/gas"
    );
    for dir in &dirs {
        let fixture = GlamsterdamBlock::from_dir(dir)?;
        let tries = IndexTries::of(&fixture)?;
        let state = run_state_consistency(&fixture)?;
        let tables = InitialValueTables::of(&state);
        let ranges = split_by_gas(&tries, parts)?;
        let outputs = run_ranges(&fixture, &tries, &tables, &ranges)
            .with_context(|| format!("fixture {}", dir.display()))?;
        aggregate(&state, &outputs).with_context(|| format!("fixture {}", dir.display()))?;
        let mut block_cycles = 0u64;
        for (i, output) in outputs.iter().enumerate() {
            let start = RangeStart::after(&outputs[..i]);
            let mut builder = program.transpiler_runner();
            if cfg!(target_arch = "x86_64") {
                builder = builder.with_jit();
            }
            let runner = builder
                .build()
                .context("failed to build the transpiler runner")?;
            let oracle = make_range_oracle(
                &fixture,
                &tries,
                &tables,
                output.transactions,
                &start,
                RunMode::RiscVRun,
            )?;
            let execution = runner
                .run_with_source(oracle)
                .context("transpiler execution failed")?;
            anyhow::ensure!(execution.reached_end, "the program did not reach the end");
            anyhow::ensure!(
                execution.receipt.output == ProgramOutput::of_range(output).hash_words(),
                "the program's output is not the hash of the range's linkage output"
            );
            block_cycles += execution.cycles_executed as u64;
            if outputs.len() > 1 {
                println!(
                    "{:>10} {:>10} {:>5} {:>9} {:>14} {:>10}",
                    "",
                    "",
                    output.transactions.len(),
                    format!("{}..{}", output.transactions.start, output.transactions.end),
                    execution.cycles_executed,
                    ""
                );
            }
        }
        let gas = fixture.block.header.inner.gas_used;
        total += block_cycles;
        total_gas += gas;
        println!(
            "{:>10} {:>10} {:>5} {:>9} {:>14} {:>10.2}",
            fixture.block.header.inner.number,
            gas,
            tries.transactions.len(),
            format!("{} part(s)", outputs.len()),
            block_cycles,
            block_cycles as f64 / gas.max(1) as f64
        );
    }
    println!(
        "{:>10} {:>10} {:>5} {:>9} {:>14} {:>10.2}",
        "total",
        total_gas,
        "",
        "",
        total,
        total as f64 / total_gas.max(1) as f64
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_block_ranges_run_the_fixtures() {
        let checked = check_path(&crate::glamsterdam::default_fixture_dir(), 1).expect("fixtures");
        assert!(checked > 0);
    }

    /// Two ranges that split a block agree with the whole block on the block's gas
    #[test]
    fn split_ranges_agree_with_the_whole_block() {
        let dirs = crate::glamsterdam::fixture_dirs(&crate::glamsterdam::default_fixture_dir())
            .expect("fixtures");
        let fixture = GlamsterdamBlock::from_dir(&dirs[0]).expect("fixture");
        let tries = IndexTries::of(&fixture).expect("tries");
        let state = run_state_consistency(&fixture).expect("pre-execution");
        let tables = InitialValueTables::of(&state);
        let count = tries.transactions.len() as u32;
        let whole = run_range(
            &fixture,
            &tries,
            &tables,
            whole_block(&tries),
            &RangeStart::default(),
        )
        .expect("whole block");
        let split = count / 2;
        let ranges = [
            TransactionRange {
                start: 0,
                end: split,
            },
            TransactionRange {
                start: split,
                end: count,
            },
        ];
        let outputs = run_ranges(&fixture, &tries, &tables, &ranges).expect("ranges");
        let [first, second] = outputs.as_slice() else {
            panic!("two outputs")
        };
        assert_eq!(second.cumulative_gas_used, whole.cumulative_gas_used);
        assert_eq!(
            second.block_execution_gas_used,
            whole.block_execution_gas_used
        );
        assert_eq!(second.block_state_gas_used, whole.block_state_gas_used);
        assert_eq!(second.block_blob_gas_used, whole.block_blob_gas_used);
        // the second range continued the first's bloom, so it ended with the block's
        assert_eq!(second.start.logs_bloom, first.bloom);
        assert_eq!(second.bloom, whole.bloom);
        assert_eq!(second.start.cumulative_gas_used, first.cumulative_gas_used);
        assert!(whole.accessed.is_full());
        let mut accessed = first.accessed.clone();
        accessed.or_assign(&second.accessed).unwrap();
        assert_eq!(accessed, whole.accessed);
        aggregate(&state, &outputs).expect("the two ranges aggregate");
    }

    /// Every fixture split into subranges by gas aggregates to the block: the
    /// tables match the pre-execution run, the gas sums link, the accessed
    /// masks cover the access list; a missing range, a wrong start gas and a
    /// range of another block are rejected
    #[test]
    fn subranges_aggregate_to_the_block() {
        let dirs = crate::glamsterdam::fixture_dirs(&crate::glamsterdam::default_fixture_dir())
            .expect("fixtures");
        let mut foreign: Option<GlamsterdamRangeOutput<Global>> = None;
        for dir in &dirs {
            let fixture = GlamsterdamBlock::from_dir(dir).expect("fixture");
            let tries = IndexTries::of(&fixture).expect("tries");
            let state = run_state_consistency(&fixture).expect("pre-execution");
            let tables = InitialValueTables::of(&state);
            let ranges = split_by_gas(&tries, 4).expect("split");
            assert_eq!(ranges[0].start, 0);
            assert_eq!(ranges.last().unwrap().end, tries.transactions.len() as u32);
            for pair in ranges.windows(2) {
                assert_eq!(pair[1].start, pair[0].end);
                assert!(!pair[0].is_empty());
            }
            let mut outputs = run_ranges(&fixture, &tries, &tables, &ranges)
                .unwrap_or_else(|e| panic!("fixture {}: {e}", dir.display()));
            for output in &outputs {
                println!("{}", report(&fixture, output));
            }
            aggregate(&state, &outputs)
                .unwrap_or_else(|e| panic!("fixture {}: {e}", dir.display()));
            println!(
                "block {}: {} ranges aggregate to the block",
                fixture.block.header.inner.number,
                outputs.len()
            );

            if outputs.len() > 1 {
                let last = outputs.pop().unwrap();
                assert!(aggregate(&state, &outputs).is_err(), "a missing range");
                let mut wrong = last;
                wrong.start.execution_gas_used += 1;
                outputs.push(wrong);
                assert!(aggregate(&state, &outputs).is_err(), "a wrong start gas");
                let mut wrong = outputs.pop().unwrap();
                wrong.start.execution_gas_used -= 1;
                // a deposit request hashing state that is not the previous range's end
                wrong.requests.start = Sha256Accumulator::ZERO;
                outputs.push(wrong);
                assert!(
                    aggregate(&state, &outputs).is_err(),
                    "a hashing state nobody ended with"
                );
                outputs.pop();
            }
            if let Some(foreign) = &foreign {
                let mut aggregation = RangeAggregation::new(PreExecutionSummary::of(&state));
                assert!(
                    aggregation.add(foreign).is_err(),
                    "a range of another block"
                );
            } else {
                foreign = outputs.into_iter().next();
            }
        }
    }
    use alloy::eips::eip7928::AccountChanges;
    use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::index_trie;
    use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::receipt::ReceiptReflection;
    use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::requests::Sha256Accumulator;
    use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::storage_model::BalStorageModel;
    use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::storage_policy::GlamsterdamStorageAccessCostModel;
    use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::witness::{
        witness_proven_transaction_count, witness_transaction_count, witness_transaction_range,
        BlockAccessIndexRange, InitialValues,
    };
    use basic_system::system_implementation::ethereum_storage_model::{
        BoxInterner, MptPreimagesOracle,
    };
    use oracle_provider::ReadWitnessSource;
    use rig::crypto::MiniDigest;
    use rig::evm_interpreter::v3::gas::COLD_ACCOUNT_ACCESS;
    use ruint::aliases::B160;
    use storage_models::common_structs::snapshottable_io::SnapshottableIo;
    use storage_models::common_structs::StorageModel;
    use zk_ee::execution_environment_type::ExecutionEnvironmentType;
    use zk_ee::reference_implementations::GlamsterdamResources;
    use zk_ee::system::resources::Ergs32;
    use zk_ee::system::{AccountDataRequest, Resource, Resources};
    use zk_ee::utils::UsizeAlignedByteBox;

    /// Every transaction and receipt of every fixture is reachable through the
    /// index tries, the count is proven, and the initial tables arrive intact
    #[test]
    fn range_witness_matches_fixtures() {
        let dirs = crate::glamsterdam::fixture_dirs(&crate::glamsterdam::default_fixture_dir())
            .expect("fixtures");
        for dir in dirs {
            let fixture = GlamsterdamBlock::from_dir(&dir).expect("fixture");
            let tries = IndexTries::of(&fixture).expect("tries must match the header");
            let count = tries.transactions.len() as u32;
            let range = TransactionRange {
                start: 0,
                end: count,
            };
            let state = run_state_consistency(&fixture).expect("pre-execution");
            assert_eq!(state.transaction_count, count);
            let tables = InitialValueTables::of(&state);
            let oracle = make_range_oracle(
                &fixture,
                &tries,
                &tables,
                range,
                &RangeStart::default(),
                RunMode::NativeRunSavingForRiscV,
            )
            .expect("oracle");
            let mut source = ReadWitnessSource::new(oracle);
            let header = &fixture.block.header.inner;
            let transactions_root = Bytes32::from_array(header.transactions_root.0);
            let receipts_root = Bytes32::from_array(header.receipts_root.0);

            assert_eq!(
                witness_transaction_count(&mut source).expect("count"),
                count
            );
            assert_eq!(
                witness_proven_transaction_count(
                    &mut source,
                    &transactions_root,
                    &receipts_root,
                    Global
                )
                .expect("proven count"),
                count
            );
            assert_eq!(
                witness_transaction_range(&mut source).expect("range"),
                range
            );
            assert_eq!(
                BlockAccessIndexRange::of(range, count).expect("access indices"),
                BlockAccessIndexRange {
                    first: 0,
                    last: count + 1
                }
            );

            let initial = InitialValues::witness(&mut source, Global).expect("initial values");
            let expected_hash =
                |bytes: &[u8]| -> Bytes32 { rig::crypto::sha3::Keccak256::digest(bytes).into() };
            assert_eq!(
                initial.hashes,
                [
                    expected_hash(state.initial_accounts_bytes()),
                    expected_hash(state.fresh_accounts_bytes()),
                    expected_hash(state.initial_slots_bytes()),
                ]
            );
            assert_eq!(initial.hashes, state.initial_values_hashes());
            assert_eq!(initial.num_accounts(), state.counts.accounts);
            assert_eq!(initial.num_slots(), state.counts.slots);
            assert_eq!(initial.accounts().count(), state.counts.accounts);
            for (index, item) in initial.accounts().enumerate() {
                let index = index as u32;
                assert_eq!(item, state.initial_account(index));
                assert_eq!(initial.is_fresh(index), state.is_fresh(index));
                assert_eq!(initial.is_fresh(index), item.is_none());
            }
            assert_eq!(initial.slots().count(), state.counts.slots);
            for (position, item) in initial.slots().enumerate() {
                assert_eq!(item, state.initial_slot(position as u32));
            }

            let mut interner = BoxInterner::with_capacity_in(1 << 22, Global);
            let mut preimages = MptPreimagesOracle(&mut source);
            let mut hasher = rig::crypto::sha3::Keccak256::new();
            let mut cumulative = 0u64;
            for (index, (tx, receipt)) in tries
                .transactions
                .iter()
                .zip(tries.receipts.iter())
                .enumerate()
            {
                let index = index as u32;
                let leaf = index_trie::lookup(
                    &transactions_root,
                    index,
                    count,
                    &mut preimages,
                    &mut interner,
                    &mut hasher,
                )
                .expect("lookup")
                .expect("transaction leaf");
                assert_eq!(leaf, tx.as_slice(), "transaction {index}");
                let leaf = index_trie::lookup(
                    &receipts_root,
                    index,
                    count,
                    &mut preimages,
                    &mut interner,
                    &mut hasher,
                )
                .expect("lookup")
                .expect("receipt leaf");
                assert_eq!(leaf, receipt.as_slice(), "receipt {index}");
                let parsed = ReceiptReflection::parse(leaf).expect("receipt");
                assert!(
                    parsed.cumulative_gas_used >= cumulative,
                    "cumulative gas decreases"
                );
                cumulative = parsed.cumulative_gas_used;
                for log in parsed.logs() {
                    let log = log.expect("log");
                    for topic in log.topics() {
                        topic.expect("topic");
                    }
                }
            }
            // Glamsterdam receipts accumulate the refund-free gas (EIP-7778 block accounting),
            // the header carries the refunded total, so the last receipt is at or above it
            assert!(
                cumulative >= header.gas_used,
                "cumulative gas of the last receipt {cumulative} below the header's {}",
                header.gas_used
            );
            // past the count: a miss, or a branch the rule rejects for that count
            assert!(matches!(
                index_trie::lookup(
                    &transactions_root,
                    count,
                    count,
                    &mut preimages,
                    &mut interner,
                    &mut hasher
                ),
                Ok(None) | Err(_)
            ));
            // a smaller count is rejected on the path of its last item or on
            // the path of index 0 (which shares the root's child 8 with the
            // indices from 128 on)
            for smaller in [count / 2, count - 1] {
                let mut rejected = false;
                for index in [0, smaller - 1] {
                    rejected |= index_trie::lookup(
                        &transactions_root,
                        index,
                        smaller,
                        &mut preimages,
                        &mut interner,
                        &mut hasher,
                    )
                    .is_err();
                }
                assert!(rejected, "count {smaller} of {count} passes the rule");
            }
            println!(
                "block {}: {} transactions and receipts opened through the tries",
                header.number, count
            );
        }
    }

    type Model =
        BalStorageModel<Global, GlamsterdamResources<()>, GlamsterdamStorageAccessCostModel, false>;

    fn address_of(account: &AccountChanges) -> B160 {
        B160::from_be_bytes(account.address.0 .0)
    }

    fn b32(value: &alloy::primitives::U256) -> Bytes32 {
        Bytes32::from_array(value.to_be_bytes::<32>())
    }

    /// A model over a fixture's access list for `range`, with the oracle it reads from
    fn model_for(
        fixture: &GlamsterdamBlock,
        tries: &IndexTries,
        range: TransactionRange,
    ) -> (Model, ReadWitnessSource) {
        let count = tries.transactions.len() as u32;
        let indices = BlockAccessIndexRange::of(range, count).expect("a range of the block");
        let tables =
            InitialValueTables::of(&run_state_consistency(fixture).expect("pre-execution"));
        let oracle = make_range_oracle(
            fixture,
            tries,
            &tables,
            range,
            &RangeStart::default(),
            RunMode::NativeRunSavingForRiscV,
        )
        .expect("oracle");
        let mut source = ReadWitnessSource::new(oracle);
        let encoding = UsizeAlignedByteBox::from_slice_in(&access_list_encoding(fixture), Global);
        let initial = InitialValues::witness(&mut source, Global).expect("initial values");
        let model = Model::new(
            encoding,
            initial,
            indices,
            GlamsterdamStorageAccessCostModel,
            Global,
        )
        .expect("model");
        (model, source)
    }

    /// Feeds the changes the access list declares at `index` through the model's API
    fn replay_index(
        model: &mut Model,
        source: &mut ReadWitnessSource,
        bal: &[AccountChanges],
        index: u32,
    ) {
        use ExecutionEnvironmentType::{NoEE, EVM};
        let mut resources = GlamsterdamResources::<()>::FORMAL_INFINITE;
        for account in bal {
            let address = address_of(account);
            for slot in &account.storage_changes {
                let key = b32(&slot.slot);
                for change in &slot.changes {
                    if change.block_access_index.0 == index as u64 {
                        model
                            .storage_write(
                                NoEE,
                                &mut resources,
                                &address,
                                &key,
                                &b32(&change.new_value),
                                source,
                            )
                            .expect("slot write");
                    }
                }
            }
            for change in &account.balance_changes {
                if change.block_access_index.0 == index as u64 {
                    let post = ruint::aliases::U256::from_be_bytes(
                        change.post_balance.to_be_bytes::<32>(),
                    );
                    model
                        .update_nominal_token_value(
                            NoEE,
                            &mut resources,
                            &address,
                            |_| Ok(post),
                            source,
                            false,
                        )
                        .expect("balance update");
                }
            }
            for change in &account.nonce_changes {
                if change.block_access_index.0 == index as u64 {
                    let current = model
                        .read_account_properties(
                            NoEE,
                            &mut resources,
                            &address,
                            AccountDataRequest::empty().with_nonce(),
                            source,
                        )
                        .expect("nonce read")
                        .nonce
                        .0;
                    assert!(change.new_nonce >= current, "nonce decreases");
                    model
                        .increment_nonce(
                            NoEE,
                            &mut resources,
                            &address,
                            change.new_nonce - current,
                            source,
                        )
                        .expect("nonce update");
                }
            }
            for change in &account.code_changes {
                if change.block_access_index.0 == index as u64 {
                    model
                        .deploy_code(
                            EVM,
                            &mut resources,
                            &address,
                            change.new_code.as_ref(),
                            source,
                        )
                        .expect("code deploy");
                }
            }
        }
    }

    /// The declared changes, replayed through the model index by index, pass the
    /// symmetric-difference check at every index, from the start of the block and
    /// from the middle of it; a wrong value, an undeclared write and an access
    /// outside the list are rejected
    #[test]
    fn storage_model_replays_access_list_changes() {
        let dirs = crate::glamsterdam::fixture_dirs(&crate::glamsterdam::default_fixture_dir())
            .expect("fixtures");
        for dir in dirs {
            let fixture = GlamsterdamBlock::from_dir(&dir).expect("fixture");
            let tries = IndexTries::of(&fixture).expect("tries");
            let count = tries.transactions.len() as u32;
            let bal = &fixture.block_access_list;
            for start in [0, count / 2] {
                let range = TransactionRange { start, end: count };
                let (mut model, mut source) = model_for(&fixture, &tries, range);
                let indices = BlockAccessIndexRange::of(range, count).unwrap();
                for index in indices.first..=indices.last {
                    model.begin_index(index).expect("index");
                    replay_index(&mut model, &mut source, bal, index);
                    model.finish_tx().unwrap_or_else(|e| {
                        panic!(
                            "index {index} of block {}: {e:?}",
                            fixture.block.header.inner.number
                        )
                    });
                }
            }

            // a slot written by a transaction to a value other than the declared one
            let (slot_index, slot_address, slot_key, slot_value) = bal
                .iter()
                .flat_map(|account| {
                    account.storage_changes.iter().flat_map(move |slot| {
                        slot.changes.iter().map(move |change| {
                            (
                                change.block_access_index.0 as u32,
                                address_of(account),
                                b32(&slot.slot),
                                b32(&change.new_value),
                            )
                        })
                    })
                })
                .find(|(index, ..)| (1..=count).contains(index))
                .expect("a storage change of a transaction");
            let range = TransactionRange {
                start: slot_index - 1,
                end: count,
            };
            let (mut model, mut source) = model_for(&fixture, &tries, range);
            model.begin_index(slot_index).expect("index");
            replay_index(&mut model, &mut source, bal, slot_index);
            let mut wrong = slot_value;
            wrong.as_u8_array_mut()[0] ^= 1;
            let mut resources = GlamsterdamResources::<()>::FORMAL_INFINITE;
            model
                .storage_write(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    &slot_address,
                    &slot_key,
                    &wrong,
                    &mut source,
                )
                .expect("write");
            assert!(
                model.finish_tx().is_err(),
                "a wrong final value must be rejected"
            );

            // a first write of the slot in a frame that is rolled back, then the
            // declared changes: one change, as declared (the rollback takes the
            // slot off the touched list, so it is not counted twice)
            let (mut model, mut source) = model_for(&fixture, &tries, range);
            model.begin_index(slot_index).expect("index");
            let snapshot = model.start_frame();
            model
                .storage_write(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    &slot_address,
                    &slot_key,
                    &wrong,
                    &mut source,
                )
                .expect("write");
            model.finish_frame(Some(&snapshot)).expect("rollback");
            replay_index(&mut model, &mut source, bal, slot_index);
            model
                .finish_tx()
                .expect("a rolled-back first write does not count");

            // an undeclared write to a slot the list only reads
            let (read_address, read_key) = bal
                .iter()
                .flat_map(|account| {
                    account
                        .storage_reads
                        .iter()
                        .map(move |key| (address_of(account), b32(key)))
                })
                .next()
                .expect("a storage read");
            let (mut model, mut source) = model_for(&fixture, &tries, range);
            model.begin_index(slot_index).expect("index");
            replay_index(&mut model, &mut source, bal, slot_index);
            let current = model
                .storage_read(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    &read_address,
                    &read_key,
                    &mut source,
                )
                .expect("read");
            let mut other = current;
            other.as_u8_array_mut()[31] ^= 1;
            model
                .storage_write(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    &read_address,
                    &read_key,
                    &other,
                    &mut source,
                )
                .expect("write");
            assert!(
                model.finish_tx().is_err(),
                "an undeclared change must be rejected"
            );

            // an address outside the list
            let (mut model, mut source) = model_for(&fixture, &tries, range);
            model.begin_index(slot_index).expect("index");
            let outside = B160::from_be_bytes([0xee; 20]);
            assert!(model
                .read_account_properties(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    &outside,
                    AccountDataRequest::empty().with_nonce(),
                    &mut source
                )
                .is_err());

            // a warm-up (a transaction access list entry) of an address or a slot outside
            // the list is accepted and warms nothing: the model holds no state for it, so an
            // access of it from the EVM still pays the cold access before being rejected
            let (mut model, mut source) = model_for(&fixture, &tries, range);
            model.begin_index(slot_index).expect("index");
            model
                .touch_account(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    &outside,
                    &mut source,
                )
                .expect("a warm-up outside the list is no access");
            model
                .storage_touch(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    &outside,
                    &slot_key,
                    &mut source,
                )
                .expect("a slot warm-up outside the list is no access");
            model
                .storage_touch(
                    ExecutionEnvironmentType::NoEE,
                    &mut resources,
                    &read_address,
                    &Bytes32::from_array([0xee; 32]),
                    &mut source,
                )
                .expect("a slot warm-up of a slot outside the list is no access");
            let mut metered = GlamsterdamResources::<()>::from_ergs(Ergs32(100_000));
            assert!(model
                .read_account_properties(
                    ExecutionEnvironmentType::EVM,
                    &mut metered,
                    &outside,
                    AccountDataRequest::empty().with_nonce(),
                    &mut source
                )
                .is_err());
            assert_eq!(
                metered.legacy_gas(),
                100_000 - COLD_ACCOUNT_ACCESS as u64,
                "the cold access of an address the warm-up did not warm"
            );
            println!(
                "block {}: access list replayed through the storage model",
                fixture.block.header.inner.number
            );
        }
    }
}
