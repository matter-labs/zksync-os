//! Ethproofs integration: fetch a mainnet block and its execution witness
//! from a Reth node, record the ZKsync OS prover input for it, prove the
//! block with `airbender-host` and (optionally) submit the proof to the
//! Ethproofs API.
//!
//! The RISC-V program is the `eth_stf` distribution built by
//! `zksync_os/dump_bin.sh --type eth-stf`. The prover backend is selected at
//! compile time: `--features gpu` for real GPU proofs, `--features proving`
//! for (very slow) CPU proofs, and the transpiler-backed dev prover otherwise.

use crate::block::Block;
use crate::live_run::rpc::{self, EthProofPayload, JsonResponse};
use airbender_host::{Program, ProveResult, Prover, Runner};
use alloy::consensus::Header;
use alloy_rpc_types_debug::ExecutionWitness;
use anyhow::Context;
use rig::alloy_rlp::Encodable;
use rig::log::{info, warn};
use rig::zksync_os_interface::traits::EncodedTx;
use rig::zksync_os_tests_common::zksync_tx::encoding::encode_alloy_rpc_tx;
use rig::Chain;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Distribution (under `zksync_os/dist/`) of the Ethereum STF program.
pub const ETHPROOFS_APP: &str = "eth_stf";
/// Same program built with full debug info (`dump_bin.sh --type eth-stf-fusaka-debug`),
/// used to symbolize flamegraphs.
pub const ETHPROOFS_DEBUG_APP: &str = "eth_stf_debug";
/// Cached prover input inside a block fixture directory.
pub const PROVER_INPUT_FILE: &str = "prover_input.bincode";

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const CONFIRMATIONS: u64 = 2;

/// Everything the guest needs for one block, decoded from RPC responses.
pub struct EthBlockInputs {
    pub block_number: u64,
    pub gas_used: u64,
    pub header: Header,
    pub transactions: Vec<EncodedTx>,
    pub withdrawals_encoding: Vec<u8>,
    pub witness: ExecutionWitness,
}

impl EthBlockInputs {
    pub fn new(block: Block, witness: ExecutionWitness) -> Self {
        let block_number = block.result.header.number;
        let gas_used = block.result.header.gas_used;
        let header: Header = block.result.header.clone().into();
        let withdrawals_encoding = if let Some(withdrawals) = block.result.withdrawals.as_ref() {
            let mut buff = vec![];
            withdrawals.encode(&mut buff);
            buff
        } else {
            Vec::new()
        };
        let transactions: Vec<EncodedTx> = block
            .result
            .transactions
            .into_transactions()
            .map(encode_alloy_rpc_tx)
            .collect();
        Self {
            block_number,
            gas_used,
            header,
            transactions,
            withdrawals_encoding,
            witness,
        }
    }

    /// Queries the Reth node for the block and its execution witness.
    pub fn from_rpc(block_number: u64, reth_endpoint: &str) -> anyhow::Result<Self> {
        Self::from_rpc_with_dump(block_number, reth_endpoint, None)
    }

    /// Like [`Self::from_rpc`]; when `dump_dir` is set the raw JSON-RPC
    /// responses are written there as `block.json` + `witness.json`, in the
    /// layout [`Self::from_dir`] (and the `eth-run` command) reads.
    pub fn from_rpc_with_dump(
        block_number: u64,
        reth_endpoint: &str,
        dump_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let start = Instant::now();
        let block = rpc::get_block(reth_endpoint, block_number)
            .context(format!("Failed to fetch block for {block_number}"))?;
        let witness = rpc::get_witness(reth_endpoint, block_number)
            .context(format!("Failed to fetch witness for {block_number}"))?;
        info!(
            "Fetched block {block_number} (gas used {}) in {:?}",
            block.result.header.gas_used,
            start.elapsed()
        );
        if let Some(dir) = dump_dir {
            std::fs::create_dir_all(dir)
                .context(format!("Failed to create dump dir {}", dir.display()))?;
            serde_json::to_writer(
                std::io::BufWriter::new(std::fs::File::create(dir.join("block.json"))?),
                &block,
            )
            .context("Failed to write block.json")?;
            serde_json::to_writer(
                std::io::BufWriter::new(std::fs::File::create(dir.join("witness.json"))?),
                &witness,
            )
            .context("Failed to write witness.json")?;
            info!("Block fixture written to {}", dir.display());
        }
        Ok(Self::new(block, witness.result))
    }

    /// Loads `block.json` + `witness.json` (raw JSON-RPC responses) from a directory.
    pub fn from_dir(dir: &Path) -> anyhow::Result<Self> {
        let block = std::fs::read_to_string(dir.join("block.json")).context(format!(
            "Failed to read {}",
            dir.join("block.json").display()
        ))?;
        let block: Block = serde_json::from_str(&block).context("Failed to parse block.json")?;
        let witness = std::fs::File::open(dir.join("witness.json")).context(format!(
            "Failed to open {}",
            dir.join("witness.json").display()
        ))?;
        let witness: JsonResponse<ExecutionWitness> =
            serde_json::from_reader(std::io::BufReader::new(witness))
                .context("Failed to parse witness.json")?;
        Ok(Self::new(block, witness.result))
    }

    /// Record the prover input: the non-determinism words the `eth_stf`
    /// RISC-V binary reads while executing this block.
    pub fn prover_input(&self) -> Vec<u32> {
        let (words, _) = Chain::<false>::record_eth_block_prover_input(
            self.transactions.clone(),
            self.witness.clone(),
            self.header.clone(),
            self.withdrawals_encoding.clone(),
        );
        words
    }
}

/// Serialized prover input, as written by `fetch-witness` and read by
/// `prove-with-witness`.
#[derive(serde::Serialize, serde::Deserialize)]
struct ProverInputFile(Vec<u32>);

fn witness_file_path(witness_output_dir: &str, block_number: u64) -> PathBuf {
    Path::new(witness_output_dir).join(format!("{block_number}_witness.bincode"))
}

pub fn write_prover_input(path: &Path, words: &[u32]) -> anyhow::Result<()> {
    let serialized =
        bincode::serde::encode_to_vec(ProverInputFile(words.to_vec()), bincode::config::standard())
            .context("Failed to serialize the prover input")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create witness output directory")?;
    }
    std::fs::write(path, &serialized).context(format!(
        "Failed to write prover input to {}",
        path.display()
    ))?;
    Ok(())
}

pub fn read_prover_input(path: &Path) -> anyhow::Result<Vec<u32>> {
    let serialized = std::fs::read(path).context(format!(
        "Failed to read prover input from {}",
        path.display()
    ))?;
    let (ProverInputFile(words), _) =
        bincode::serde::decode_from_slice(&serialized, bincode::config::standard())
            .context("Failed to deserialize the prover input")?;
    Ok(words)
}

/// Loads the `eth_stf` program distribution (`zksync_os/dist/eth_stf`, or the
/// override pointed to by `OVERRIDE_ZKSYNC_OS_PATH`).
pub fn load_program() -> anyhow::Result<Program> {
    load_program_for(ETHPROOFS_APP)
}

/// Loads an arbitrary program distribution under `zksync_os/dist/<app>`.
pub fn load_program_for(app: &str) -> anyhow::Result<Program> {
    let dist_dir = rig::chain::get_zksync_os_dist_dir(&Some(app.to_string()));
    Program::load(&dist_dir).with_context(|| {
        format!(
            "failed to load the `{app}` program from {}; build it with `zksync_os/dump_bin.sh --type eth-stf-fusaka` (or `eth-stf-fusaka-debug` for `{ETHPROOFS_DEBUG_APP}`)",
            dist_dir.display()
        )
    })
}

/// Prover input for a block fixture directory: read from the cached
/// `prover_input.bincode` when present, otherwise recorded natively and cached.
pub fn load_or_record_prover_input(
    block_dir: &Path,
    inputs: &EthBlockInputs,
) -> anyhow::Result<Vec<u32>> {
    let path = block_dir.join(PROVER_INPUT_FILE);
    if path.exists() {
        let words = read_prover_input(&path)?;
        info!(
            "Loaded {} cached prover input words from {}",
            words.len(),
            path.display()
        );
        return Ok(words);
    }
    let start = Instant::now();
    let words = inputs.prover_input();
    info!(
        "Recorded {} prover input words for block {} in {:?}",
        words.len(),
        inputs.block_number,
        start.elapsed()
    );
    write_prover_input(&path, &words)?;
    Ok(words)
}

/// Collects `count` blocks with their execution witnesses, storing each one as
/// `<output_dir>/<block>/{block.json,witness.json,prover_input.bincode}`, and
/// returns the block numbers that were collected successfully.
///
/// Blocks are visited from `start_block` (default: the confirmed head)
/// downwards. A block whose native replay fails (the STF panics, for example
/// on a not yet supported feature) is reported and skipped, so the routine
/// keeps going until `count` blocks succeeded or `3 * count` blocks were tried.
pub fn ethproofs_collect(
    reth_endpoint: &str,
    count: u64,
    start_block: Option<u64>,
    output_dir: &Path,
) -> anyhow::Result<Vec<u64>> {
    anyhow::ensure!(count > 0, "count must be positive");
    let start = match start_block {
        Some(start) => start,
        None => rpc::get_block_number(reth_endpoint)?.saturating_sub(CONFIRMATIONS),
    };
    let mut collected = Vec::with_capacity(count as usize);
    let mut failed = Vec::new();
    let mut block_number = start;
    let max_attempts = 3 * count;
    let mut attempts = 0;
    while (collected.len() as u64) < count && attempts < max_attempts {
        attempts += 1;
        let current = block_number;
        block_number = block_number.saturating_sub(1);
        match collect_one_block(reth_endpoint, current, output_dir) {
            Ok(()) => collected.push(current),
            Err(err) => {
                warn!("Skipping block {current}: {err:#}");
                failed.push(current);
            }
        }
    }
    collected.sort_unstable();
    println!(
        "Collected {} block(s): {:?}; failed: {:?}",
        collected.len(),
        collected,
        failed
    );
    anyhow::ensure!(
        collected.len() as u64 == count,
        "collected only {} of {count} blocks",
        collected.len()
    );
    Ok(collected)
}

fn collect_one_block(
    reth_endpoint: &str,
    block_number: u64,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let dir = output_dir.join(block_number.to_string());
    let inputs = if dir.join("witness.json").exists() {
        info!("Block {block_number} already fetched in {}", dir.display());
        EthBlockInputs::from_dir(&dir)?
    } else {
        EthBlockInputs::from_rpc_with_dump(block_number, reth_endpoint, Some(&dir))?
    };
    // The native replay panics on blocks the STF cannot process; turn that
    // into an error so the collection can move on to the next block.
    let words = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        load_or_record_prover_input(&dir, &inputs)
    })) {
        Ok(words) => words?,
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown panic".to_string());
            anyhow::bail!("native replay panicked: {message}");
        }
    };
    println!(
        "Collected block {block_number}: {} txs, {} gas, {} prover input words -> {}",
        inputs.transactions.len(),
        inputs.gas_used,
        words.len(),
        dir.display()
    );
    Ok(())
}

/// Options for [`ethproofs_flamegraph`].
pub struct FlamegraphOptions {
    /// Program distribution to run (defaults to [`ETHPROOFS_DEBUG_APP`]).
    pub app: String,
    /// Output SVG path.
    pub output: PathBuf,
    /// Collect one stack sample every `sampling_rate` cycles.
    pub sampling_rate: usize,
    /// Reverse the stack order (leaf functions at the root, i.e. a bottom-up graph).
    pub inverse: bool,
    /// Serve the guest with the witness-parsing oracle instead of the recorded
    /// responses. The replay only works while the guest asks for exactly the
    /// hints of the natively recorded run.
    pub live_oracle: bool,
}

/// Runs a block fixture on the transpiler with the stack profiler enabled and
/// writes a flamegraph symbolized against the app's debug ELF. Returns the
/// executed cycle count.
pub fn ethproofs_flamegraph(block_dir: &Path, options: &FlamegraphOptions) -> anyhow::Result<u64> {
    let inputs = EthBlockInputs::from_dir(block_dir)?;
    let words = load_or_record_prover_input(block_dir, &inputs)?;
    let program = load_program_for(&options.app)?;
    if let Some(parent) = options.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let runner = program
        .transpiler_runner()
        .with_flamegraph(airbender_host::FlamegraphConfig {
            output: options.output.clone(),
            sampling_rate: options.sampling_rate,
            inverse: options.inverse,
            elf_path: Some(program.app_elf().to_path_buf()),
        })
        .build()
        .context("failed to build transpiler runner")?;
    let start = Instant::now();
    let execution = if options.live_oracle {
        // see `ethproofs_compare_oracles` on why it is not the "native" oracle
        let oracle = Chain::<false>::make_eth_block_oracle(
            inputs.transactions.clone(),
            inputs.witness.clone(),
            inputs.header.clone(),
            inputs.withdrawals_encoding.clone(),
        );
        runner.run_with_source(oracle)
    } else {
        runner.run(&words)
    }
    .context("transpiler execution with profiling failed")?;
    anyhow::ensure!(execution.reached_end, "program did not reach the end");
    anyhow::ensure!(
        execution.receipt.output.iter().any(|word| *word != 0),
        "program output is all zeroes, the block execution failed inside the guest"
    );
    println!(
        "Block {} ({} gas): {} cycles, profiled in {:?} (1 sample / {} cycles), flamegraph written to {}",
        inputs.block_number,
        inputs.gas_used,
        execution.cycles_executed,
        start.elapsed(),
        options.sampling_rate,
        options.output.display()
    );
    Ok(execution.cycles_executed as u64)
}

/// Runs the given blocks forward with the opcode-sequence tracer and prints the
/// most executed statically adjacent opcode pairs and triples over all of them.
pub fn ethproofs_opcode_sequences(
    block_dirs: &[PathBuf],
    top: usize,
    output: Option<&Path>,
) -> anyhow::Result<()> {
    use rig::forward_system::system::tracers::opcode_sequences::EvmOpcodeSequenceTracer;
    let mut merged = EvmOpcodeSequenceTracer::default();
    for block_dir in block_dirs {
        let inputs = EthBlockInputs::from_dir(block_dir)?;
        let mut tracer = EvmOpcodeSequenceTracer::default();
        let start = Instant::now();
        Chain::<false>::run_eth_block_forward_with_tracer(
            inputs.transactions.clone(),
            inputs.witness.clone(),
            inputs.header.clone(),
            inputs.withdrawals_encoding.clone(),
            &mut tracer,
        );
        println!(
            "Block {} ({} gas): {} EVM steps traced in {:?}",
            inputs.block_number,
            inputs.gas_used,
            tracer.total_steps,
            start.elapsed()
        );
        merged.merge(&tracer);
    }
    merged.print_top(top);
    if let Some(path) = output {
        merged.write_csv(path)?;
        println!("Sequences written to {}", path.display());
    }
    Ok(())
}

/// Timing of one transpiler run.
#[derive(Clone, Debug)]
pub struct OracleRunTiming {
    /// Time to construct the non-determinism source (witness parsing, MPT
    /// reconstruction, ...) before the guest starts.
    pub setup: Duration,
    /// Guest execution time.
    pub execution: Duration,
    pub cycles: u64,
    pub output: [u32; 8],
}

/// Executes the same block twice on the transpiler: once with the real oracle
/// (`ZkEENonDeterminismSource`, which parses the execution witness and answers
/// every guest query on the fly) and once with a replay source that serves the
/// pre-recorded word stream and ignores guest writes. Returns the per-run
/// timings `(oracle, replay)`, each repeated `runs` times.
pub fn ethproofs_compare_oracles(
    block_dir: &Path,
    runs: usize,
    app: &str,
) -> anyhow::Result<(Vec<OracleRunTiming>, Vec<OracleRunTiming>)> {
    use airbender_host::raw::QuasiUARTSource;

    anyhow::ensure!(runs > 0, "runs must be positive");
    let inputs = EthBlockInputs::from_dir(block_dir)?;
    let words = load_or_record_prover_input(block_dir, &inputs)?;
    let program = load_program_for(app)?;
    let mut builder = program.transpiler_runner();
    if cfg!(target_arch = "x86_64") {
        builder = builder.with_jit();
    }
    let runner = builder
        .build()
        .context("failed to build transpiler runner")?;

    let mut oracle_timings = Vec::with_capacity(runs);
    for run in 0..runs {
        let setup_start = Instant::now();
        // The transpiler runs the guest in its own RAM, so the callable oracles
        // must read hint structures through `RamPeek` (the "native" variants used
        // for prover-input recording dereference host pointers instead).
        let oracle = Chain::<false>::make_eth_block_oracle(
            inputs.transactions.clone(),
            inputs.witness.clone(),
            inputs.header.clone(),
            inputs.withdrawals_encoding.clone(),
        );
        let setup = setup_start.elapsed();
        let exec_start = Instant::now();
        let execution = runner
            .run_with_source(oracle)
            .context("transpiler execution with the witness oracle failed")?;
        let timing = OracleRunTiming {
            setup,
            execution: exec_start.elapsed(),
            cycles: execution.cycles_executed as u64,
            output: execution.receipt.output,
        };
        info!(
            "oracle run {run}: setup {:?}, execution {:?}, {} cycles",
            timing.setup, timing.execution, timing.cycles
        );
        oracle_timings.push(timing);
    }

    let mut replay_timings = Vec::with_capacity(runs);
    for run in 0..runs {
        let setup_start = Instant::now();
        let source = QuasiUARTSource::new_with_reads(words.clone());
        let setup = setup_start.elapsed();
        let exec_start = Instant::now();
        let execution = runner
            .run_with_source(source)
            .context("transpiler execution with the replay source failed")?;
        let timing = OracleRunTiming {
            setup,
            execution: exec_start.elapsed(),
            cycles: execution.cycles_executed as u64,
            output: execution.receipt.output,
        };
        info!(
            "replay run {run}: setup {:?}, execution {:?}, {} cycles",
            timing.setup, timing.execution, timing.cycles
        );
        replay_timings.push(timing);
    }

    for (a, b) in oracle_timings.iter().zip(replay_timings.iter()) {
        anyhow::ensure!(
            a.output == b.output && a.cycles == b.cycles,
            "oracle and replay runs diverged: {:?}/{} cycles vs {:?}/{} cycles",
            a.output,
            a.cycles,
            b.output,
            b.cycles
        );
    }
    anyhow::ensure!(
        oracle_timings[0].output.iter().any(|word| *word != 0),
        "program output is all zeroes, the block execution failed inside the guest"
    );

    let best = |timings: &[OracleRunTiming]| {
        let setup = timings.iter().map(|t| t.setup).min().unwrap();
        let execution = timings.iter().map(|t| t.execution).min().unwrap();
        (setup, execution)
    };
    let (oracle_setup, oracle_exec) = best(&oracle_timings);
    let (replay_setup, replay_exec) = best(&replay_timings);
    println!(
        "Block {} ({} gas, {} cycles, {} prover input words), best of {runs} runs, {}:",
        inputs.block_number,
        inputs.gas_used,
        oracle_timings[0].cycles,
        words.len(),
        if cfg!(target_arch = "x86_64") {
            "JIT"
        } else {
            "interpreter"
        }
    );
    println!(
        "  witness oracle: setup {:?} + execution {:?} = {:?}",
        oracle_setup,
        oracle_exec,
        oracle_setup + oracle_exec
    );
    println!(
        "  replay source:  setup {:?} + execution {:?} = {:?}",
        replay_setup,
        replay_exec,
        replay_setup + replay_exec
    );
    println!(
        "  execution overhead of the witness oracle: {:+.2?} ({:+.1}%)",
        oracle_exec.checked_sub(replay_exec).unwrap_or_default(),
        (oracle_exec.as_secs_f64() / replay_exec.as_secs_f64() - 1.0) * 100.0
    );
    Ok((oracle_timings, replay_timings))
}

/// Builds the prover for the `eth_stf` program with the compile-time selected
/// backend: GPU (`gpu` feature), CPU (`proving` feature) or the transpiler-backed
/// dev prover.
pub fn build_prover(worker_threads: Option<usize>) -> anyhow::Result<Box<dyn Prover>> {
    let program = load_program()?;
    #[cfg(feature = "gpu")]
    {
        info!("Setting up GPU prover...");
        let prover = program
            .gpu_prover()
            .with_config(
                airbender_host::GpuProverConfig::default().maybe_worker_threads(worker_threads),
            )
            .build()
            .context("failed to build GPU prover")?;
        info!("Done setting up GPU prover.");
        Ok(Box::new(prover))
    }
    #[cfg(all(feature = "proving", not(feature = "gpu")))]
    {
        let prover = program
            .cpu_prover()
            .maybe_worker_threads(worker_threads)
            .build()
            .context("failed to build CPU prover")?;
        Ok(Box::new(prover))
    }
    #[cfg(not(any(feature = "proving", feature = "gpu")))]
    {
        let _ = worker_threads;
        warn!(
            "neither `gpu` nor `proving` feature is enabled: using the dev prover (no real proofs)"
        );
        let prover = program
            .dev_prover()
            .build()
            .context("failed to build dev prover")?;
        Ok(Box::new(prover))
    }
}

/// Executes the program on the transpiler (JIT on x86_64, interpreter
/// elsewhere) to count the program's own cycles. Real proofs at a recursion
/// level only report the cycle count of the last recursion verifier, so the
/// Ethproofs `proving_cycles` field is taken from this run instead.
pub fn count_program_cycles(program: &Program, prover_input: &[u32]) -> anyhow::Result<u64> {
    let mut builder = program.transpiler_runner();
    if cfg!(target_arch = "x86_64") {
        builder = builder.with_jit();
    }
    let runner = builder
        .build()
        .context("failed to build transpiler runner")?;
    let execution = runner
        .run(prover_input)
        .context("transpiler execution failed")?;
    anyhow::ensure!(
        execution.reached_end,
        "program did not reach the end within {} cycles",
        execution.cycles_executed
    );
    anyhow::ensure!(
        execution.receipt.output.iter().any(|word| *word != 0),
        "program output is all zeroes, the block execution failed inside the guest"
    );
    Ok(execution.cycles_executed as u64)
}

/// Proof of one block together with the timings Ethproofs wants reported.
pub struct BlockProof {
    pub block_number: u64,
    pub gas_used: u64,
    pub prove_result: ProveResult,
    /// Cycles the program itself executed (see [`count_program_cycles`]).
    pub program_cycles: u64,
    /// Prover-input recording + proving, i.e. everything that is not RPC.
    pub proving_time: Duration,
}

impl BlockProof {
    /// bincode-serialized, base64-encoded proof envelope as submitted to Ethproofs.
    pub fn encoded_proof(&self) -> anyhow::Result<String> {
        use base64::Engine;
        let serialized =
            bincode::serde::encode_to_vec(&self.prove_result.proof, bincode::config::standard())
                .context("Failed to serialize the program proof")?;
        Ok(base64::engine::general_purpose::STANDARD.encode(&serialized))
    }
}

/// Records the prover input for the block, proves it and counts its cycles.
pub fn prove_block(
    prover: &dyn Prover,
    program: &Program,
    inputs: &EthBlockInputs,
) -> anyhow::Result<BlockProof> {
    let start = Instant::now();
    let prover_input = inputs.prover_input();
    info!(
        "Recorded {} prover input words for block {} in {:?}",
        prover_input.len(),
        inputs.block_number,
        start.elapsed()
    );
    let prove_result = prove_words(prover, &prover_input)?;
    let proving_time = start.elapsed();
    info!(
        "Proved block {} in {:?}: {}",
        inputs.block_number,
        proving_time,
        prove_result.proof.debug_info()
    );

    let program_cycles = count_program_cycles(program, &prover_input)?;
    info!("Block {} took {program_cycles} cycles", inputs.block_number);

    Ok(BlockProof {
        block_number: inputs.block_number,
        gas_used: inputs.gas_used,
        prove_result,
        program_cycles,
        proving_time,
    })
}

fn prove_words(prover: &dyn Prover, prover_input: &[u32]) -> anyhow::Result<ProveResult> {
    let prove_result = prover.prove(prover_input).context("proving failed")?;
    anyhow::ensure!(
        prove_result.receipt.output.iter().any(|word| *word != 0),
        "proof output is all zeroes, the block execution failed inside the guest"
    );
    Ok(prove_result)
}

/// Runs the whole pipeline for a block stored on disk (`block.json` +
/// `witness.json`): the `invoke_single_block` test entry point.
pub fn prove_single_block_from_dir(
    block_dir: &Path,
    worker_threads: Option<usize>,
) -> anyhow::Result<BlockProof> {
    let inputs = EthBlockInputs::from_dir(block_dir)?;
    let program = load_program()?;
    let prover = build_prover(worker_threads)?;
    prove_block(&*prover, &program, &inputs)
}

/// Fetches a block from the Reth node, records its prover input (optionally
/// writing it to `<witness_output_dir>/<block>_witness.bincode`) and returns
/// the words with the time spent outside of RPC.
pub fn ethproofs_run(
    block_number: u64,
    reth_endpoint: &str,
    witness_output_dir: Option<&str>,
    dump_dir: Option<&str>,
) -> anyhow::Result<(Vec<u32>, Duration)> {
    let inputs =
        EthBlockInputs::from_rpc_with_dump(block_number, reth_endpoint, dump_dir.map(Path::new))?;
    let start = Instant::now();
    info!(
        "Running block: {block_number} ({} transactions, gas used {})",
        inputs.transactions.len(),
        inputs.gas_used
    );
    let words = inputs.prover_input();
    let duration = start.elapsed();
    info!("Prover input for block {block_number} recorded in {duration:?}");
    if let Some(dir) = witness_output_dir {
        let path = witness_file_path(dir, block_number);
        write_prover_input(&path, &words)?;
        info!("Prover input written to {}", path.display());
    }
    Ok((words, duration))
}

/// Follows the chain head and records the prover input for every new block.
pub fn ethproofs_live_run(reth_endpoint: &str) -> anyhow::Result<()> {
    let mut next = rpc::get_block_number(reth_endpoint)?.saturating_sub(CONFIRMATIONS);

    ethproofs_run(next, reth_endpoint, None, None)?;

    loop {
        let head = rpc::get_block_number(reth_endpoint)?.saturating_sub(CONFIRMATIONS);
        if head > next {
            for n in (next + 1)..=head {
                ethproofs_run(n, reth_endpoint, None, None)?;
            }
            next = head;
        } else {
            sleep(POLL_INTERVAL);
        }
    }
}

pub fn ethproofs_fetch_witness(
    reth_endpoint: &str,
    block_number: u64,
    witness_output_dir: &str,
) -> anyhow::Result<()> {
    let (_, duration) = ethproofs_run(block_number, reth_endpoint, Some(witness_output_dir), None)?;
    println!(
        "Fetched witness for block {} in {:?}, written to {}",
        block_number,
        duration,
        witness_file_path(witness_output_dir, block_number).display()
    );
    Ok(())
}

/// Proves a block from a prover input file written by `fetch-witness`. The
/// proof envelope is written next to it as `<witness_input>.proof.bin`.
pub fn ethproofs_prove_with_witness(
    witness_input: &str,
    worker_threads: Option<usize>,
) -> anyhow::Result<()> {
    let witness_path = Path::new(witness_input);
    let prover_input = read_prover_input(witness_path)?;
    println!(
        "Generating proof for {} prover input words from {}",
        prover_input.len(),
        witness_input
    );

    let prover = build_prover(worker_threads)?;
    let start = Instant::now();
    let prove_result = prove_words(&*prover, &prover_input)?;
    let total_proof_time = start.elapsed();

    let serialized =
        bincode::serde::encode_to_vec(&prove_result.proof, bincode::config::standard())
            .context("Failed to serialize the program proof")?;
    let proof_path = witness_path.with_extension("proof.bin");
    std::fs::write(&proof_path, &serialized)
        .context(format!("Failed to write proof to {}", proof_path.display()))?;

    println!(
        "Generated proof in {:?} ({}), proof size: {} bytes, written to {}",
        total_proof_time,
        prove_result.proof.debug_info(),
        serialized.len(),
        proof_path.display()
    );
    Ok(())
}

/// Proves blocks live and submits them to Ethproofs (when a connector is given).
///
/// `block_selector` is `(prover_id, block_mod)`: only blocks with
/// `block_number % block_mod == prover_id` are proven, so several provers can
/// share the chain.
pub fn ethproofs_with_proofs(
    reth_endpoint: &str,
    connector: Option<EthProofsConnector>,
    block_selector: (u64, u64),
    worker_threads: Option<usize>,
) -> anyhow::Result<()> {
    let program = load_program()?;
    let prover = build_prover(worker_threads)?;

    let mut next = 0;

    loop {
        let head = rpc::get_block_number(reth_endpoint)?;
        let head = EthProofsConnector::select_block(head, block_selector);
        if head > next {
            println!("Generating proof for block {}", head);
            let inputs = EthBlockInputs::from_rpc(head, reth_endpoint)?;
            let block_proof = prove_block(&*prover, &program, &inputs)?;
            let encoded_proof = block_proof.encoded_proof()?;
            println!(
                "Block {} proved in {:?} ({} cycles, {} bytes of proof)",
                head,
                block_proof.proving_time,
                block_proof.program_cycles,
                encoded_proof.len()
            );

            if let Some(connector) = connector.as_ref() {
                connector.send_proof(
                    head,
                    &encoded_proof,
                    block_proof.proving_time,
                    block_proof.program_cycles,
                )?;
            }

            next = head;
        } else {
            sleep(POLL_INTERVAL);
        }
    }
}

pub struct EthProofsConnector {
    pub staging: bool,
    pub auth_token: String,
    pub cluster_id: u64,
    pub url: String,
}

impl EthProofsConnector {
    pub fn new(staging: bool, auth_token: String, cluster_id: u64) -> Self {
        let url = if staging {
            "https://staging--ethproofs.netlify.app/api/v0/".to_string()
        } else {
            "https://ethproofs.netlify.app/api/v0/".to_string()
        };
        Self {
            staging,
            auth_token,
            cluster_id,
            url,
        }
    }

    /// Selects the block to prove for `candidate_block` under the
    /// `(prover_id, block_mod)` schedule: the latest block at or below the
    /// candidate whose number is `prover_id` modulo `block_mod`.
    pub fn select_block(candidate_block: u64, (prover_id, block_mod): (u64, u64)) -> u64 {
        // This is the block that we should pick.
        let selected_block = candidate_block - (candidate_block % block_mod) + prover_id;

        // But if it turns out to be larger than candidate_block, we need to wait for the next round.
        // And we'll return the previous round's block number to indicate that.
        if selected_block > candidate_block {
            // Return block from the previous round.
            return selected_block - block_mod;
        }
        selected_block
    }

    pub fn send_proof(
        &self,
        block_number: u64,
        serialized_proof: &str,
        time_spent: Duration,
        cycles: u64,
    ) -> anyhow::Result<()> {
        println!(
            "Sending proof for block {} to ethproofs server ({}), time spent: {:?}, proof size: {} bytes",
            block_number,
            if self.staging { "staging" } else { "production" },
            time_spent,
            serialized_proof.len()
        );
        let payload = EthProofPayload {
            block_number,
            cluster_id: self.cluster_id,
            proving_time: time_spent.as_millis() as u64,
            proving_cycles: cycles,
            proof: serialized_proof.to_string(),
            verifier_id: "None".to_string(),
        };
        let response = rpc::send_ethproofs(
            &format!("{}proofs/proved", self.url),
            self.auth_token.clone(),
            payload,
        )?;
        println!("Response from server: {}", response);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_block_selection() {
        assert_eq!(EthProofsConnector::select_block(100, (0, 10)), 100);
        assert_eq!(EthProofsConnector::select_block(100, (5, 10)), 95);
        assert_eq!(EthProofsConnector::select_block(100, (9, 10)), 99);
        assert_eq!(EthProofsConnector::select_block(105, (0, 10)), 100);
        assert_eq!(EthProofsConnector::select_block(105, (5, 10)), 105);
        assert_eq!(EthProofsConnector::select_block(105, (9, 10)), 99);
    }
}
