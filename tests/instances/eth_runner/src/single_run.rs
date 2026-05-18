use crate::block::Block;
use crate::block_hashes::BlockHashes;
use crate::calltrace::CallTrace;
use crate::native_model::compute_ratio;
use crate::post_check::post_check;
use crate::prestate::{populate_prestate, DiffTrace, PrestateTrace};
use crate::receipts::{BlockReceipts, TransactionReceipt};
use rig::chain::BlockExtraStats;
use rig::forward_system::system::system_types::ForwardRunningSystem;
use rig::forward_system::system::tracers::evm_opcode_stats::EvmOpcodeStatsTracer;
use rig::forward_system::system::tracers::pair::Pair;
use rig::forward_system::system::tracers::precompile_stats::PrecompileStatsTracer;
use rig::log::info;
use rig::*;
use std::fs::{self, File};
use std::io::BufReader;
use zk_ee::system::tracer::{NopTracer, Tracer};
use zk_ee::system::validator::NopTxValidator;
use zksync_os_interface::traits::EncodedTx;
use zksync_os_interface::types::BlockOutput;

#[allow(clippy::too_many_arguments)]
fn run<const RANDOMIZED: bool>(
    mut chain: Chain<RANDOMIZED>,
    block_context: BlockContext,
    block_number: u64,
    _miner: alloy::primitives::Address,
    ps_trace: PrestateTrace,
    transactions: Vec<EncodedTx>,
    receipts: Vec<TransactionReceipt>,
    diff_trace: DiffTrace,
    calltrace: CallTrace,
    block_hashes: Option<BlockHashes>,
    witness_output_dir: Option<String>,
    flamegraph: Option<String>,
    opcode_stats: bool,
) -> anyhow::Result<()> {
    chain.set_last_block_number(block_number - 1);

    if let Some(block_hashes) = block_hashes {
        chain.set_block_hashes(block_hashes.into_array(block_number))
    }

    let prestate_cache = populate_prestate(&mut chain, ps_trace, &calltrace);

    let output_path = witness_output_dir.map(|dir| {
        let mut suffix = block_number.to_string();
        suffix.push_str("_witness");
        std::path::Path::new(&dir).join(suffix)
    });
    let flamegraph = flamegraph.map(|p| rig::FlamegraphOptions::new(p.into()));
    let run_config = rig::chain::RunConfig {
        witness_output_file: output_path,
        flamegraph,
        do_riscv_run: true,
        app: Some("evm_replay".to_string()),
        check_storage_diff_hashes: true,
        ..Default::default()
    };

    let dump_opcode_tracer = |tracer: &EvmOpcodeStatsTracer<ForwardRunningSystem>| {
        tracer.print_stats();
        if let Ok(path) = std::env::var("OPCODE_STATS_PATH") {
            tracer
                .write_csv(std::path::Path::new(&path))
                .expect("Failed to write opcode stats CSV");
            info!("Opcode stats written to {path}");
        }
        if let Ok(dir) = std::env::var("OPCODE_SAMPLES_DIR") {
            tracer
                .dump_samples(std::path::Path::new(&dir))
                .expect("Failed to dump opcode samples");
            info!("Opcode samples dumped to {dir}");
        }
    };

    let dump_precompile_tracer = |tracer: &PrecompileStatsTracer<ForwardRunningSystem>| {
        tracer.print_stats();
        if let Ok(path) = std::env::var("PRECOMPILE_STATS_PATH") {
            tracer
                .write_csv(std::path::Path::new(&path))
                .expect("Failed to write precompile stats CSV");
            info!("Precompile stats written to {path}");
        }
        if let Ok(dir) = std::env::var("PRECOMPILE_SAMPLES_DIR") {
            tracer
                .dump_samples(std::path::Path::new(&dir))
                .expect("Failed to dump precompile samples");
            info!("Precompile samples dumped to {dir}");
        }
    };

    let precompile_stats_enabled = std::env::var("PRECOMPILE_STATS_PATH").is_ok()
        || std::env::var("PRECOMPILE_SAMPLES_DIR").is_ok();

    let (output, stats, pubdata) = match (opcode_stats, precompile_stats_enabled) {
        (true, true) => {
            let mut composite = Pair::new(
                EvmOpcodeStatsTracer::<ForwardRunningSystem>::default(),
                PrecompileStatsTracer::<ForwardRunningSystem>::default(),
            );
            let result = run_with_tracer(
                &mut chain,
                transactions,
                block_context,
                run_config,
                &mut composite,
            );
            dump_opcode_tracer(&composite.a);
            dump_precompile_tracer(&composite.b);
            result
        }
        (true, false) => {
            let mut tracer = EvmOpcodeStatsTracer::<ForwardRunningSystem>::default();
            let result = run_with_tracer(
                &mut chain,
                transactions,
                block_context,
                run_config,
                &mut tracer,
            );
            dump_opcode_tracer(&tracer);
            result
        }
        (false, true) => {
            let mut p_tracer = PrecompileStatsTracer::<ForwardRunningSystem>::default();
            let result = run_with_tracer(
                &mut chain,
                transactions,
                block_context,
                run_config,
                &mut p_tracer,
            );
            dump_precompile_tracer(&p_tracer);
            result
        }
        (false, false) => run_with_tracer(
            &mut chain,
            transactions,
            block_context,
            run_config,
            &mut NopTracer::default(),
        ),
    };

    let _ratio = compute_ratio(stats);

    // Append the pubdata size to the cycle-marker bench file so
    // `compare_pubdata.py` can A/B it across a PR. Best-effort: only fires
    // when MARKER_PATH is set (i.e. the caller asked for bench output), and
    // silently no-ops if the file isn't writable. Matches the parsing
    // contract in `bench_scripts/compare_pubdata.py`.
    //
    // Also emits the deflate-compressed sizes at levels 1 (fast) and 9
    // (best) — a post-STF measurement to evaluate the "compressed envelope"
    // optimization (#1 in `pubdata_optimization.md`) without changing the
    // STF's emitted blob. The bench A/B pipeline can extend the parser later
    // to surface these alongside the uncompressed size.
    if let Ok(path) = std::env::var("MARKER_PATH") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
        {
            let _ = writeln!(f, "pubdata_bytes: {}", pubdata.len());
            let _ = writeln!(
                f,
                "pubdata_bytes_deflate1: {}",
                rig::pubdata_compression::deflate(&pubdata, 1).len()
            );
            let _ = writeln!(
                f,
                "pubdata_bytes_deflate9: {}",
                rig::pubdata_compression::deflate(&pubdata, 9).len()
            );
        }
    }

    post_check(output, receipts, diff_trace, prestate_cache).unwrap();

    Ok(())
}

fn run_with_tracer<const RANDOMIZED: bool>(
    chain: &mut Chain<RANDOMIZED>,
    transactions: Vec<EncodedTx>,
    block_context: BlockContext,
    run_config: rig::chain::RunConfig,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> (BlockOutput, BlockExtraStats, Vec<u8>) {
    // Allow benchmarking to opt into a non-default DA commitment scheme. The
    // bench currently runs two passes per block: the default `BlobsAndPubdataKeccak256`
    // (which emits a placeholder zero-hash blob plus a keccak commitment over
    // pubdata) and `BlobsZKsyncOS` (which actually exercises the
    // `BlobCommitmentGenerator` and the `blob_versioned_hash` cycle marker).
    // Falls back to the rig default when unset.
    //
    // NOTE: the literal string `BENCH_DA_SCHEME` is used as a `grep -q`
    // fallback target by `.github/workflows/bench.yml` — if this env var
    // name is renamed, update the workflow too.
    let da_commitment_scheme = std::env::var("BENCH_DA_SCHEME").ok().map(|s| {
        use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
        match s.as_str() {
            "keccak" | "blobs_and_pubdata_keccak256" => {
                DACommitmentScheme::BlobsAndPubdataKeccak256
            }
            "blobs_zksync_os" | "blobs" => DACommitmentScheme::BlobsZKsyncOS,
            "empty_no_da" => DACommitmentScheme::EmptyNoDA,
            other => panic!("Unknown BENCH_DA_SCHEME: {other}"),
        }
    });

    let (output, stats, _, pubdata) = chain
        .run_block_with_extra_stats(
            transactions,
            Some(block_context),
            da_commitment_scheme,
            Some(run_config),
            tracer,
            &mut NopTxValidator,
        )
        .unwrap();

    (output, stats, pubdata)
}

#[allow(clippy::too_many_arguments)]
pub fn single_run(
    block_dir: String,
    block_hashes: Option<String>,
    randomized: bool,
    witness_output_dir: Option<String>,
    chain_id: Option<u64>,
    single_tx: Option<u64>,
    flamegraph: Option<String>,
    opcode_stats: bool,
) -> anyhow::Result<()> {
    use std::path::Path;

    anyhow::ensure!(
        witness_output_dir.is_none() || flamegraph.is_none(),
        "--witness-output-dir and --flamegraph cannot be used together"
    );

    let dir = Path::new(&block_dir);
    let block = fs::read_to_string(dir.join("block.json"))?;
    // TODO: ensure there are no calls to unsupported precompiles
    let calltrace_file = File::open(dir.join("calltrace.json"))?;
    let calltrace_reader = BufReader::new(calltrace_file);
    let receipts = fs::read_to_string(dir.join("receipts.json"))?;
    let ps_file = File::open(dir.join("prestatetrace.json"))?;
    let ps_reader = BufReader::new(ps_file);
    let ps_trace: PrestateTrace = serde_json::from_reader(ps_reader)?;
    let receipts: BlockReceipts = serde_json::from_str(&receipts).expect("valid receipts JSON");
    let diff_file = File::open(dir.join("difftrace.json"))?;
    let diff_reader = BufReader::new(diff_file);
    let diff_trace: DiffTrace = serde_json::from_reader(diff_reader)?;
    let block_hashes: Option<BlockHashes> = block_hashes.map(|path| {
        let hashes = fs::read_to_string(&path).expect("valid block hashes path");
        serde_json::from_str(&hashes).expect("valid block hashes JSON")
    });

    let calltrace: CallTrace = serde_json::from_reader(calltrace_reader)?;
    let block: Block = serde_json::from_str(&block).expect("valid block JSON");
    let block_number = block.result.header.number;
    info!("Running block: {block_number}");
    info!("Block gas used: {}", block.result.header.gas_used);
    // assert!(block.result.header.gas_used <= 11_000_000);
    let miner = block.result.header.beneficiary;

    let block_context = block.get_block_context();
    let (transactions, skipped, _) = block.get_transactions(&calltrace, single_tx);

    let receipts = receipts
        .result
        .into_iter()
        .enumerate()
        .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
        .collect();

    let ps_trace = PrestateTrace {
        result: ps_trace
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    let diff_trace = DiffTrace {
        result: diff_trace
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    let calltrace = CallTrace {
        result: calltrace
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    if randomized {
        let chain = Chain::empty_randomized(Some(chain_id.unwrap_or(1)));
        run(
            chain,
            block_context,
            block_number,
            miner,
            ps_trace,
            transactions,
            receipts,
            diff_trace,
            calltrace,
            block_hashes,
            witness_output_dir,
            flamegraph,
            opcode_stats,
        )
    } else {
        let chain = Chain::empty(Some(1));
        run(
            chain,
            block_context,
            block_number,
            miner,
            ps_trace,
            transactions,
            receipts,
            diff_trace,
            calltrace,
            block_hashes,
            witness_output_dir,
            flamegraph,
            opcode_stats,
        )
    }
}

pub fn eth_run(block_dir: String) -> anyhow::Result<()> {
    use rig::alloy_rlp::Encodable;
    use rig::zksync_os_tests_common::zksync_tx::encoding::encode_alloy_rpc_tx;
    use std::path::Path;

    let dir = Path::new(&block_dir);
    let block = fs::read_to_string(dir.join("block.json"))?;
    let witness_file = File::open(dir.join("witness.json"))?;
    let witness_reader = BufReader::new(witness_file);

    let block: Block = serde_json::from_str(&block)?;

    // Parse witness JSON - it has a "result" wrapper
    #[derive(serde::Deserialize)]
    struct WitnessWrapper {
        result: alloy_rpc_types_debug::ExecutionWitness,
    }
    let witness_wrapper: WitnessWrapper = serde_json::from_reader(witness_reader)?;
    let witness = witness_wrapper.result;

    let transactions: Vec<EncodedTx> = block
        .result
        .transactions
        .clone()
        .into_transactions()
        .map(encode_alloy_rpc_tx)
        .collect();

    let mut chain = Chain::empty(Some(1));

    chain.set_last_block_number(block.result.number() - 1);

    let header = block.result.header.clone().into();
    let withdrawals_encoding = if let Some(withdrawals) = block.result.withdrawals.clone() {
        let mut buff = vec![];
        withdrawals.encode(&mut buff);

        buff
    } else {
        Vec::new()
    };

    let _ = chain.run_eth_block(transactions, witness, header, withdrawals_encoding);
    Ok(())
}
