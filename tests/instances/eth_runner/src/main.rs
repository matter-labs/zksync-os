#![feature(slice_as_array)]

use clap::{Parser, Subcommand};
mod block;
mod block_hashes;
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
    },
    // Run a single block from JSON files
    SingleRun {
        /// Path to the block JSON file
        #[arg(long)]
        block: String,

        /// Path to the call trace JSON file
        #[arg(long)]
        calltrace: String,

        /// Path to the prestate trace JSON file
        #[arg(long)]
        prestatetrace: String,

        /// PAth to the diff trace JSON file
        #[arg(long)]
        difftrace: String,

        /// Path to the block receipts trace JSON file
        #[arg(long)]
        receipts: String,

        /// Path to the block hashes JSON file (optional)
        #[arg(long)]
        block_hashes: Option<String>,

        /// If set, the leaves of the tree are put in random
        /// positions to emulate real-world costs
        #[arg(long, action = clap::ArgAction::SetTrue)]
        randomized: bool,
    },
}

fn main() -> anyhow::Result<()> {
    rig::init_logger();
    let cli = Cli::parse();
    match cli.command {
        Command::SingleRun {
            block,
            calltrace,
            prestatetrace,
            difftrace,
            receipts,
            block_hashes,
            randomized,
        } => crate::single_run::single_run(
            block,
            calltrace,
            receipts,
            prestatetrace,
            difftrace,
            block_hashes,
            randomized,
        ),
        Command::LiveRun {
            start_block,
            end_block,
            endpoint,
            db,
        } => live_run::live_run(start_block, end_block, endpoint, db),
    }
}
