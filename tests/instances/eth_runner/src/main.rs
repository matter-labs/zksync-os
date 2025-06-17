#![feature(slice_as_array)]

use block_hashes::BlockHashes;
use clap::Parser;
use post_check::post_check;
use prestate::{populate_prestate, DiffTrace, PrestateTrace};
use rig::*;
use std::fs::{self, File};
use std::io::BufReader;

mod block;
mod block_hashes;
mod post_check;
mod prestate;
mod receipts;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
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
}

#[allow(clippy::too_many_arguments)]
fn run<const RANDOMIZED: bool>(
    mut chain: Chain<RANDOMIZED>,
    block_context: BlockContext,
    block_number: u64,
    miner: alloy::primitives::Address,
    ps_trace: PrestateTrace,
    transactions: Vec<Vec<u8>>,
    receipts: Vec<receipts::TransactionReceipt>,
    diff_trace: DiffTrace,
    block_hashes: Option<BlockHashes>,
) -> anyhow::Result<()> {
    chain.set_last_block_number(block_number - 1);

    if let Some(block_hashes) = block_hashes {
        chain.set_block_hashes(block_hashes.into_array(block_number))
    }

    let prestate_cache = populate_prestate(&mut chain, ps_trace);

    let output = chain.run_block(transactions, Some(block_context), None);

    post_check(
        output,
        receipts,
        diff_trace,
        prestate_cache,
        ruint::aliases::B160::from_be_bytes(miner.into()),
    );

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let block = fs::read_to_string(&args.block)?;
    // TODO: ensure there are no calls to unsupported precompiles
    let _calltrace = fs::read_to_string(&args.calltrace)?;
    let receipts = fs::read_to_string(&args.receipts)?;
    let ps_file = File::open(&args.prestatetrace)?;
    let ps_reader = BufReader::new(ps_file);
    let ps_trace: PrestateTrace = serde_json::from_reader(ps_reader)?;
    let receipts: receipts::BlockReceipts =
        serde_json::from_str(&receipts).expect("valid receipts JSON");
    let diff_file = File::open(&args.difftrace)?;
    let diff_reader = BufReader::new(diff_file);
    let diff_trace: DiffTrace = serde_json::from_reader(diff_reader)?;
    let block_hashes: Option<BlockHashes> = args.block_hashes.map(|path| {
        let hashes = fs::read_to_string(&path).expect("valid block hashes path");
        serde_json::from_str(&hashes).expect("valid block hashes JSON")
    });

    let block: block::Block = serde_json::from_str(&block).expect("valid block JSON");
    let block_number = block.result.header.number;
    println!("Block gas used: {}", block.result.header.gas_used);
    // assert!(block.result.header.gas_used <= 11_000_000);
    let miner = block.result.header.miner;

    let block_context = block.get_block_context();
    let (transactions, skipped) = block.get_transactions();

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

    if args.randomized {
        let chain = Chain::empty_randomized(Some(1));
        run(
            chain,
            block_context,
            block_number,
            miner,
            ps_trace,
            transactions,
            receipts,
            diff_trace,
            block_hashes,
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
            block_hashes,
        )
    }
}
