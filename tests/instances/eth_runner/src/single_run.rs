use crate::block::Block;
use crate::block_hashes::BlockHashes;
use crate::calltrace::CallTrace;
use crate::native_model::compute_ratio;
use crate::post_check::post_check;
use crate::prestate::{populate_prestate, DiffTrace, PrestateTrace};
use crate::receipts::{BlockReceipts, TransactionReceipt};
use rig::log::info;
use rig::*;
use std::fs::{self, File};
use std::io::BufReader;

#[allow(clippy::too_many_arguments)]
fn run<const RANDOMIZED: bool>(
    mut chain: Chain<RANDOMIZED>,
    block_context: BlockContext,
    block_number: u64,
    miner: alloy::primitives::Address,
    ps_trace: PrestateTrace,
    transactions: Vec<Vec<u8>>,
    receipts: Vec<TransactionReceipt>,
    diff_trace: DiffTrace,
    calltrace: CallTrace,
    block_hashes: Option<BlockHashes>,
    witness_output_dir: Option<String>,
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

    let (output, stats) = chain.run_block_with_extra_stats(
        transactions,
        Some(block_context),
        None,
        output_path,
        Some("evm_replay".to_string()),
    );

    let _ratio = compute_ratio(stats);

    post_check(
        output,
        receipts,
        diff_trace,
        prestate_cache,
        ruint::aliases::B160::from_be_bytes(miner.into()),
    )
    .unwrap();

    Ok(())
}

pub fn single_run(
    block_dir: String,
    block_hashes: Option<String>,
    randomized: bool,
    witness_output_dir: Option<String>,
    chain_id: Option<u64>,
) -> anyhow::Result<()> {
    use std::path::Path;
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
    let (transactions, skipped) = block.get_transactions(&calltrace);

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
        )
    }
}
