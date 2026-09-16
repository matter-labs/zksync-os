#![recursion_limit = "1024"]

use clap::{Parser, Subcommand};
mod block;
mod block_hashes;
mod calltrace;
mod ethproofs;
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
        } => ethproofs::ethproofs_flamegraph(
            std::path::Path::new(&block_dir),
            &ethproofs::FlamegraphOptions {
                app,
                output: output.into(),
                sampling_rate,
                inverse,
            },
        )
        .map(|_| ()),
        Command::EthproofsCompareOracles {
            block_dir,
            runs,
            app,
        } => ethproofs::ethproofs_compare_oracles(std::path::Path::new(&block_dir), runs, &app)
            .map(|_| ()),
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
}
