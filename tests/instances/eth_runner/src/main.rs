#![recursion_limit = "1024"]
#![feature(allocator_api)]

use clap::{Parser, Subcommand};
mod block;
mod block_hashes;
mod calltrace;
mod ethproofs;
mod glamsterdam;
mod glamsterdam_aggregation;
mod glamsterdam_consistency;
mod glamsterdam_range;
mod glamsterdam_state;
mod live_run;
mod native_model;
mod post_check;
mod prestate;
mod receipts;
mod single_run;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run a range of blocks live from RPC
    LiveRun {
        #[arg(long)]
        start_block: u64,
        #[arg(long)]
        end_block: u64,
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        db: String,
        #[arg(long)]
        witness_output_dir: Option<String>,
        #[arg(long)]
        skip_successful: bool,
        #[arg(long)]
        persist_all: bool,
        #[arg(long)]
        slack_webhook: Option<String>,
        #[arg(long)]
        single_tx: Option<u64>,
        #[arg(long)]
        only_forward: bool,
        #[arg(long)]
        backup_endpoint: Option<String>,
    },
    // Run a single block from JSON files
    SingleRun {
        /// Path to the block JSON file
        #[arg(long)]
        block_dir: String,
        /// Path to the block hashes JSON file (optional)
        #[arg(long)]
        block_hashes: Option<String>,
        /// If set, the leaves of the tree are put in random
        /// positions to emulate real-world costs
        #[arg(long, action = clap::ArgAction::SetTrue)]
        randomized: bool,
        /// If set, will run prover input generation and dump it
        /// to the desired path.
        #[arg(long)]
        witness_output_dir: Option<String>,
        #[arg(long)]
        chain_id: Option<u64>,
        #[arg(long)]
        single_tx: Option<u64>,
        /// If set, generates a flamegraph SVG at the given path.
        /// Cannot be used together with --witness-output-dir.
        #[arg(long, conflicts_with = "witness_output_dir")]
        flamegraph: Option<String>,
        /// Enable per-opcode EVM statistics collection.
        /// Adds overhead; omit when only block-level benchmarks are needed.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        opcode_stats: bool,
    },
    // Export block ratios from DB
    ExportRatios {
        #[arg(long)]
        db: String,
        #[arg(long)]
        path: Option<String>,
    },
    // Show failed blocks
    ShowStatus {
        #[arg(long)]
        db: String,
    },
    // Run a single block using eth_run
    EthRun {
        /// Path to the block directory
        #[arg(long)]
        block_dir: String,
    },
    /// Fetch Glamsterdam block fixtures (block with transactions, execution
    /// witness, block access list, raw header) from a Geth node into
    /// `<output-dir>/<block>/`. Only finalized blocks are accepted.
    GlamsterdamFetch {
        /// Geth JSON-RPC endpoint with the `debug` namespace enabled.
        #[arg(long)]
        endpoint: String,
        /// First block of the range.
        #[arg(long)]
        first_block: u64,
        /// Number of consecutive blocks to fetch.
        #[arg(long, default_value_t = 1)]
        count: u64,
        /// Where to store the fixtures (defaults to `glamsterdam_blocks/` in this crate).
        #[arg(long)]
        output_dir: Option<String>,
    },
    /// Load Glamsterdam fixtures and check their internal consistency (header
    /// RLP and hash, block access list hash, access indices, parent header).
    GlamsterdamCheck {
        /// One fixture directory, or a directory of fixture directories
        /// (defaults to `glamsterdam_blocks/` in this crate).
        #[arg(long)]
        block_dir: Option<String>,
    },
    /// Run the Glamsterdam state consistency check (header, parent, block access
    /// list applied to the state) natively over stored fixtures.
    GlamsterdamStateConsistency {
        /// One fixture directory, or a directory of fixture directories
        /// (defaults to `glamsterdam_blocks/` in this crate).
        #[arg(long)]
        block_dir: Option<String>,
    },
    /// Diff our EVM steps of one transaction of a Glamsterdam fixture against the
    /// node's `debug_traceTransaction` struct logs (stack/memory/storage disabled).
    GlamsterdamTraceDiff {
        #[arg(long)]
        block_dir: String,
        /// The transaction's index in the block
        #[arg(long)]
        tx: u32,
        /// The node trace (JSON-RPC response)
        #[arg(long)]
        trace: String,
    },
    /// Run the Glamsterdam fixtures as ranges on the RISC-V simulator with the
    /// `glamsterdam_tx_range` program (`binaries/build.sh`) and report the cycles.
    GlamsterdamRangeCycles {
        /// One fixture directory, or a directory of fixture directories
        /// (defaults to `glamsterdam_blocks/` in this crate).
        #[arg(long)]
        block_dir: Option<String>,
        /// The program to run: a dist directory (relative to the workspace root), or
        /// the name of a dist under `zksync_os/dist`.
        #[arg(long, default_value = "binaries/glamsterdam_tx_range/dist")]
        app: String,
        /// Split every block into this many ranges by gas (one: the whole block).
        #[arg(long, default_value_t = 1)]
        parts: u32,
    },
    /// Check the self-consistency of the Glamsterdam programs over stored fixtures:
    /// every program natively with the oracle responses recorded, then on the RISC-V
    /// simulator from the recording alone, with the public outputs checked against
    /// the native linkage outputs and the ranges aggregated.
    GlamsterdamSelfConsistency {
        /// One fixture directory, or a directory of fixture directories
        /// (defaults to `glamsterdam_blocks/` in this crate).
        #[arg(long)]
        block_dir: Option<String>,
        /// Split every block into this many ranges by gas.
        #[arg(long, default_value_t = 3)]
        parts: u32,
        /// The pre-execution program's dist directory.
        #[arg(long, default_value = glamsterdam_consistency::PRE_EXECUTION_APP)]
        pre_execution_app: String,
        /// The transaction range program's dist directory.
        #[arg(long, default_value = glamsterdam_consistency::TX_RANGE_APP)]
        tx_range_app: String,
        /// The `glamsterdam_aggregation` program (a dist directory)
        #[arg(long, default_value = glamsterdam_consistency::AGGREGATION_APP)]
        aggregation_app: String,
    },
    /// Follow Sepolia: fetch every finalized block from a Geth node as it comes and run
    /// the self-consistency check over it (the long-running form of
    /// `glamsterdam-self-consistency`).
    GlamsterdamFollow {
        /// Geth JSON-RPC endpoint with the `debug` namespace enabled.
        #[arg(long)]
        endpoint: String,
        /// The first block (defaults to the finalized head).
        #[arg(long)]
        first_block: Option<u64>,
        /// How many blocks to check (forever by default).
        #[arg(long)]
        count: Option<u64>,
        /// Where the fixtures are fetched to (defaults to `glamsterdam_follow/` in this crate).
        #[arg(long)]
        work_dir: Option<String>,
        /// Keep the fetched fixtures instead of removing each after its check.
        #[arg(long, default_value_t = false)]
        keep: bool,
        /// Split every block into this many ranges by gas.
        #[arg(long, default_value_t = 3)]
        parts: u32,
        #[arg(long, default_value = glamsterdam_consistency::PRE_EXECUTION_APP)]
        pre_execution_app: String,
        #[arg(long, default_value = glamsterdam_consistency::TX_RANGE_APP)]
        tx_range_app: String,
        /// The `glamsterdam_aggregation` program (a dist directory)
        #[arg(long, default_value = glamsterdam_consistency::AGGREGATION_APP)]
        aggregation_app: String,
    },
    /// Run the transactions of the Glamsterdam fixtures as ranges against their
    /// block access lists and receipts (host run) and aggregate the ranges.
    GlamsterdamRange {
        /// One fixture directory, or a directory of fixture directories
        /// (defaults to `glamsterdam_blocks/` in this crate).
        #[arg(long)]
        block_dir: Option<String>,
        /// Split every block into this many ranges by gas (one: the whole block).
        #[arg(long, default_value_t = 1)]
        parts: u32,
    },
    /// Run the Glamsterdam state consistency check on the RISC-V simulator over
    /// stored fixtures and print the cycles per block. Needs the
    /// `glamsterdam_pre_execution` program (`binaries/build.sh`), or any dist built
    /// from it (`--app`).
    GlamsterdamStateConsistencyCycles {
        /// One fixture directory, or a directory of fixture directories
        /// (defaults to `glamsterdam_blocks/` in this crate).
        #[arg(long)]
        block_dir: Option<String>,
        /// The program to run: a dist directory (relative to the workspace root), or
        /// the name of a dist under `zksync_os/dist`.
        #[arg(long, default_value = "binaries/glamsterdam_pre_execution/dist")]
        app: String,
    },
    /// Profile the Glamsterdam state consistency check on the RISC-V simulator
    /// over one fixture and write a flamegraph (the `--app` dist must carry
    /// debug info, see `zksync_os/debug_symbols.toml`).
    GlamsterdamStateConsistencyFlamegraph {
        /// The fixture directory
        #[arg(long)]
        block_dir: String,
        /// The program distribution under `zksync_os/dist` to run.
        #[arg(long, default_value = "glamsterdam_state_consistency_debug")]
        app: String,
        /// Output SVG path
        #[arg(long)]
        output: String,
        /// One stack sample every this many cycles
        #[arg(long, default_value_t = 100)]
        sampling_rate: usize,
        /// Reverse the stack order (leaf functions at the root)
        #[arg(long, default_value_t = false)]
        inverse: bool,
    },
    /// Fetch an Ethereum block + execution witness from a Reth node and record
    /// its prover input (Ethproofs flow, no proving).
    EthproofsRun {
        #[arg(long)]
        block_number: u64,
        #[arg(long)]
        reth_endpoint: String,
        /// If set, the prover input is written to `<dir>/<block>_witness.bincode`.
        #[arg(long)]
        witness_output_dir: Option<String>,
        /// If set, the raw `block.json` + `witness.json` RPC responses are
        /// written there (a fixture for `eth-run` / `invoke_single_block`).
        #[arg(long)]
        dump_dir: Option<String>,
    },
    /// Follow the chain head and record the prover input of every new block.
    EthproofsLiveRun {
        #[arg(long)]
        reth_endpoint: String,
    },
    /// Prove Ethereum blocks live and submit the proofs to Ethproofs.
    EthproofsWithProofs {
        #[arg(long)]
        reth_endpoint: String,
        /// If staging is set, then proofs will be sent to staging server and we pick next available block.
        /// If not set, then proofs will be sent to production server and we every 100th block.
        #[arg(long)]
        staging: bool,
        #[arg(long)]
        auth_token: String,
        #[arg(long)]
        cluster_id: u64,
        /// If set, will select blocks where (block_number % block_mod) == prover_id
        /// If not set, will pick 100th block in production and 10th block in staging.
        #[arg(long)]
        block_mod: Option<u64>,
        #[arg(long)]
        prover_id: Option<u64>,
        /// Prover worker threads (GPU replay / CPU proving threads).
        #[arg(long)]
        worker_threads: Option<usize>,
    },
    /// Prove Ethereum blocks live without submitting anything.
    EthproofsWithProofsNoSubmission {
        #[arg(long)]
        reth_endpoint: String,
        /// If set, will select blocks where (block_number % block_mod) == prover_id
        /// If not set, will pick every 100th block.
        #[arg(long)]
        block_mod: Option<u64>,
        #[arg(long)]
        prover_id: Option<u64>,
        #[arg(long)]
        worker_threads: Option<usize>,
    },
    /// Fetch a block from a Reth node and store its prover input on disk.
    FetchWitness {
        #[arg(long)]
        reth_endpoint: String,
        #[arg(long)]
        block_number: u64,
        #[arg(long)]
        witness_output_dir: String,
    },
    /// Prove a block from a prover input file written by `fetch-witness`.
    ProveWithWitness {
        #[arg(long)]
        witness_input: String,
        #[arg(long)]
        worker_threads: Option<usize>,
    },
    /// Prove a stored block fixture (`block.json` + `witness.json`) with the
    /// compile-time selected prover and report the cycle count.
    EthproofsProveBlock {
        #[arg(long)]
        block_dir: String,
        #[arg(long)]
        worker_threads: Option<usize>,
    },
    /// Fetch consecutive blocks with their execution witnesses and record their
    /// prover inputs into `<output-dir>/<block>/`.
    EthproofsCollect {
        #[arg(long)]
        reth_endpoint: String,
        #[arg(long, default_value_t = 16)]
        count: u64,
        /// Highest block to try; defaults to the confirmed head. Blocks are
        /// walked downwards, skipping the ones the STF cannot replay.
        #[arg(long)]
        start_block: Option<u64>,
        #[arg(long)]
        output_dir: String,
    },
    /// Run a collected block on the transpiler with stack sampling and write a
    /// flamegraph symbolized against the debug ELF.
    EthproofsFlamegraph {
        #[arg(long)]
        block_dir: String,
        #[arg(long)]
        output: String,
        /// Program distribution under `zksync_os/dist/` (built with debug info).
        #[arg(long, default_value = "eth_stf_debug")]
        app: String,
        /// One stack sample every N cycles.
        #[arg(long, default_value_t = 100)]
        sampling_rate: usize,
        /// Reverse stack order (bottom-up graph: leaf functions at the root).
        #[arg(long, action = clap::ArgAction::SetTrue)]
        inverse: bool,
        /// Answer the guest's queries with the witness-parsing oracle instead of
        /// replaying the recorded responses. Needed when the guest asks for other
        /// hints than the natively recorded run did (e.g. an experimental build).
        #[arg(long, action = clap::ArgAction::SetTrue)]
        live_oracle: bool,
    },
    /// Run collected blocks forward and report the most executed statically
    /// adjacent opcode pairs and triples (instruction fusing candidates).
    EthproofsOpcodeSequences {
        /// Block directories (`<dir>/{block.json,witness.json}`), repeatable.
        #[arg(long, required = true)]
        block_dir: Vec<String>,
        /// How many sequences of each length to print.
        #[arg(long, default_value_t = 40)]
        top: usize,
        /// Optional CSV with every sequence and its count.
        #[arg(long)]
        output: Option<String>,
    },
    /// Time the steps of simulating a collected block (JSON parsing, oracle or
    /// replay source preparation, runner construction, execution).
    EthproofsSimulationTiming {
        #[arg(long)]
        block_dir: String,
        #[arg(long, default_value_t = 2)]
        runs: usize,
        #[arg(long, default_value = "eth_stf")]
        app: String,
    },
    /// Write a collected block as the inputs zilkworm's `fetch` builds its bundle from:
    /// `block<N>.json`, `executionWitness<N>.json` and `blockRlp<N>.json` (the raw block RLP,
    /// encoded from the block JSON) under `<out-dir>/<N>/`.
    EthproofsExportZilkworm {
        #[arg(long)]
        block_dir: String,
        #[arg(long)]
        out_dir: String,
    },
    /// Run a collected block with the witness-parsing oracle and with a replay
    /// source, and report the runtime difference.
    EthproofsCompareOracles {
        #[arg(long)]
        block_dir: String,
        #[arg(long, default_value_t = 3)]
        runs: usize,
        #[arg(long, default_value = "eth_stf")]
        app: String,
    },
}

fn main() -> anyhow::Result<()> {
    rig::init_logger();
    let cli = Cli::parse();
    match cli.command {
        Command::SingleRun {
            block_dir,
            block_hashes,
            randomized,
            witness_output_dir,
            chain_id,
            single_tx,
            flamegraph,
            opcode_stats,
        } => crate::single_run::single_run(
            block_dir,
            block_hashes,
            randomized,
            witness_output_dir,
            chain_id,
            single_tx,
            flamegraph,
            opcode_stats,
        ),
        Command::LiveRun {
            start_block,
            end_block,
            endpoint,
            db,
            witness_output_dir,
            skip_successful,
            persist_all,
            slack_webhook,
            single_tx,
            only_forward,
            backup_endpoint,
        } => live_run::live_run(
            start_block,
            end_block,
            endpoint,
            db,
            witness_output_dir,
            skip_successful,
            persist_all,
            slack_webhook,
            single_tx,
            only_forward,
            backup_endpoint,
        ),
        Command::ExportRatios { db, path } => live_run::export_block_ratios(db, path),
        Command::ShowStatus { db } => live_run::show_status(db),
        Command::EthRun { block_dir } => crate::single_run::eth_run(block_dir),
        Command::EthproofsRun {
            block_number,
            reth_endpoint,
            witness_output_dir,
            dump_dir,
        } => ethproofs::ethproofs_run(
            block_number,
            &reth_endpoint,
            witness_output_dir.as_deref(),
            dump_dir.as_deref(),
        )
        .map(|_| ()),
        Command::EthproofsLiveRun { reth_endpoint } => {
            ethproofs::ethproofs_live_run(&reth_endpoint)
        }
        Command::EthproofsWithProofs {
            reth_endpoint,
            staging,
            auth_token,
            cluster_id,
            block_mod,
            prover_id,
            worker_threads,
        } => {
            let block_mod = block_mod.unwrap_or(if staging { 10 } else { 100 });
            let prover_id = prover_id.unwrap_or(0);
            let connector = ethproofs::EthProofsConnector::new(staging, auth_token, cluster_id);
            ethproofs::ethproofs_with_proofs(
                &reth_endpoint,
                Some(connector),
                (prover_id, block_mod),
                worker_threads,
            )
        }
        Command::EthproofsWithProofsNoSubmission {
            reth_endpoint,
            block_mod,
            prover_id,
            worker_threads,
        } => ethproofs::ethproofs_with_proofs(
            &reth_endpoint,
            None,
            (prover_id.unwrap_or(0), block_mod.unwrap_or(100)),
            worker_threads,
        ),
        Command::FetchWitness {
            reth_endpoint,
            block_number,
            witness_output_dir,
        } => ethproofs::ethproofs_fetch_witness(&reth_endpoint, block_number, &witness_output_dir),
        Command::ProveWithWitness {
            witness_input,
            worker_threads,
        } => ethproofs::ethproofs_prove_with_witness(&witness_input, worker_threads),
        Command::EthproofsProveBlock {
            block_dir,
            worker_threads,
        } => {
            let proof = ethproofs::prove_single_block_from_dir(
                std::path::Path::new(&block_dir),
                worker_threads,
            )?;
            println!(
                "block {} ({} gas): {} cycles, proved in {:?}: {}",
                proof.block_number,
                proof.gas_used,
                proof.program_cycles,
                proof.proving_time,
                proof.prove_result.proof.debug_info()
            );
            Ok(())
        }
        Command::GlamsterdamFetch {
            endpoint,
            first_block,
            count,
            output_dir,
        } => {
            let output_dir = output_dir
                .map(std::path::PathBuf::from)
                .unwrap_or_else(glamsterdam::default_fixture_dir);
            let fetched = glamsterdam::fetch(&endpoint, first_block, count, &output_dir)?;
            println!(
                "Fetched {} block(s) into {}: {fetched:?}",
                fetched.len(),
                output_dir.display()
            );
            Ok(())
        }
        Command::GlamsterdamTraceDiff {
            block_dir,
            tx,
            trace,
        } => {
            let fixture =
                glamsterdam::GlamsterdamBlock::from_dir(std::path::Path::new(&block_dir))?;
            let tries = glamsterdam_range::IndexTries::of(&fixture)?;
            glamsterdam_range::trace_diff(&fixture, &tries, tx, std::path::Path::new(&trace))
        }
        Command::GlamsterdamRangeCycles {
            block_dir,
            app,
            parts,
        } => {
            let block_dir = block_dir
                .map(std::path::PathBuf::from)
                .unwrap_or_else(glamsterdam::default_fixture_dir);
            glamsterdam_range::run_on_simulator(&block_dir, &app, parts)
        }
        Command::GlamsterdamSelfConsistency {
            block_dir,
            parts,
            pre_execution_app,
            tx_range_app,
            aggregation_app,
        } => {
            let block_dir = block_dir
                .map(std::path::PathBuf::from)
                .unwrap_or_else(glamsterdam::default_fixture_dir);
            let programs = glamsterdam_consistency::Programs::load(
                &pre_execution_app,
                &tx_range_app,
                &aggregation_app,
            )?;
            let reports = glamsterdam_consistency::check_path(&block_dir, &programs, parts)?;
            let total: u64 = reports.iter().map(|r| r.total_cycles()).sum();
            let gas: u64 = reports.iter().map(|r| r.gas_used).sum();
            println!(
                "{} block(s) self-consistent: {total} cycles over {gas} gas ({:.2}/gas)",
                reports.len(),
                total as f64 / gas.max(1) as f64
            );
            Ok(())
        }
        Command::GlamsterdamFollow {
            endpoint,
            first_block,
            count,
            work_dir,
            keep,
            parts,
            pre_execution_app,
            tx_range_app,
            aggregation_app,
        } => {
            let work_dir = work_dir.map(std::path::PathBuf::from).unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("glamsterdam_follow")
            });
            let programs = glamsterdam_consistency::Programs::load(
                &pre_execution_app,
                &tx_range_app,
                &aggregation_app,
            )?;
            let summary = glamsterdam_consistency::follow(
                &endpoint,
                first_block,
                count,
                &work_dir,
                &programs,
                parts,
                keep,
            )?;
            println!(
                "{} block(s) self-consistent, {} diverging {:?}; tables in {}",
                summary.reports.len(),
                summary.failed.len(),
                summary.failed,
                work_dir.display()
            );
            println!("{}", summary.coverage.line());
            if summary.failed.is_empty() {
                Ok(())
            } else {
                anyhow::bail!("{} block(s) diverged", summary.failed.len())
            }
        }
        Command::GlamsterdamRange { block_dir, parts } => {
            let block_dir = block_dir
                .map(std::path::PathBuf::from)
                .unwrap_or_else(glamsterdam::default_fixture_dir);
            let checked = glamsterdam_range::check_path(&block_dir, parts)?;
            println!(
                "Transaction ranges hold for {checked} fixture(s) under {}",
                block_dir.display()
            );
            Ok(())
        }
        Command::GlamsterdamStateConsistency { block_dir } => {
            let block_dir = block_dir
                .map(std::path::PathBuf::from)
                .unwrap_or_else(glamsterdam::default_fixture_dir);
            let checked = glamsterdam_state::check_path(&block_dir)?;
            println!(
                "State consistency holds for {checked} fixture(s) under {}",
                block_dir.display()
            );
            Ok(())
        }
        Command::GlamsterdamStateConsistencyCycles { block_dir, app } => {
            let block_dir = block_dir
                .map(std::path::PathBuf::from)
                .unwrap_or_else(glamsterdam::default_fixture_dir);
            glamsterdam_state::run_on_simulator(&block_dir, &app)
        }
        Command::GlamsterdamStateConsistencyFlamegraph {
            block_dir,
            app,
            output,
            sampling_rate,
            inverse,
        } => {
            glamsterdam_state::flamegraph(
                std::path::Path::new(&block_dir),
                &app,
                std::path::Path::new(&output),
                sampling_rate,
                inverse,
            )?;
            Ok(())
        }
        Command::GlamsterdamCheck { block_dir } => {
            let block_dir = block_dir
                .map(std::path::PathBuf::from)
                .unwrap_or_else(glamsterdam::default_fixture_dir);
            let checked = glamsterdam::check_path(&block_dir)?;
            println!("Checked {checked} fixture(s) under {}", block_dir.display());
            Ok(())
        }
        Command::EthproofsCollect {
            reth_endpoint,
            count,
            start_block,
            output_dir,
        } => ethproofs::ethproofs_collect(
            &reth_endpoint,
            count,
            start_block,
            std::path::Path::new(&output_dir),
        )
        .map(|_| ()),
        Command::EthproofsFlamegraph {
            block_dir,
            output,
            app,
            sampling_rate,
            inverse,
            live_oracle,
        } => ethproofs::ethproofs_flamegraph(
            std::path::Path::new(&block_dir),
            &ethproofs::FlamegraphOptions {
                app,
                output: output.into(),
                sampling_rate,
                inverse,
                live_oracle,
            },
        )
        .map(|_| ()),
        Command::EthproofsOpcodeSequences {
            block_dir,
            top,
            output,
        } => ethproofs::ethproofs_opcode_sequences(
            &block_dir
                .iter()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>(),
            top,
            output.as_deref().map(std::path::Path::new),
        ),
        Command::EthproofsSimulationTiming {
            block_dir,
            runs,
            app,
        } => ethproofs::ethproofs_simulation_timing(std::path::Path::new(&block_dir), runs, &app),
        Command::EthproofsCompareOracles {
            block_dir,
            runs,
            app,
        } => ethproofs::ethproofs_compare_oracles(std::path::Path::new(&block_dir), runs, &app)
            .map(|_| ()),
        Command::EthproofsExportZilkworm { block_dir, out_dir } => {
            ethproofs::ethproofs_export_zilkworm(
                std::path::Path::new(&block_dir),
                std::path::Path::new(&out_dir),
            )
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    /// Ethproofs-style end-to-end run of one mainnet block from the checked-in
    /// fixture (`block.json` + `witness.json`): record the prover input for
    /// the `eth_stf` program, prove it with the compile-time selected prover
    /// (dev prover by default, `--features gpu` for real GPU proofs) and count
    /// the program cycles.
    ///
    /// Requires `zksync_os/dist/eth_stf` built with the BPO2 blob schedule
    /// (`zksync_os/dump_bin.sh --type eth-stf-fusaka`) and the matching host
    /// features: `--features rig/eth_stf,fusaka-bpo-2`.
    #[test]
    fn invoke_single_block() {
        rig::init_logger();
        let block_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("25990253");
        let proof =
            crate::ethproofs::prove_single_block_from_dir(&block_dir, None).expect("must succeed");
        println!(
            "block {} ({} gas): {} cycles, proved in {:?}: {}",
            proof.block_number,
            proof.gas_used,
            proof.program_cycles,
            proof.proving_time,
            proof.prove_result.proof.debug_info()
        );
        assert!(proof.program_cycles > 0);
    }

    /// Every Glamsterdam fixture under `glamsterdam_blocks/` parses with the
    /// current types and is internally consistent: header RLP and hash, block
    /// access list hash, access indices, parent header in the witness. See
    /// `glamsterdam::GlamsterdamBlock::check`.
    #[test]
    fn glamsterdam_fixtures_are_consistent() {
        rig::init_logger();
        let fixture_dir = crate::glamsterdam::default_fixture_dir();
        let checked =
            crate::glamsterdam::check_path(&fixture_dir).expect("fixtures must be consistent");
        assert!(checked > 0);
    }

    /// The Glamsterdam state consistency check (parent state root -> header
    /// state root through the block access list) holds on every stored fixture.
    #[test]
    fn glamsterdam_state_consistency_holds() {
        rig::init_logger();
        let fixture_dir = crate::glamsterdam::default_fixture_dir();
        let checked = crate::glamsterdam_state::check_path(&fixture_dir)
            .expect("state consistency must hold");
        assert!(checked > 0);
    }
}
