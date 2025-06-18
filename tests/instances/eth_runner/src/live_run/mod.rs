use alloy::primitives::U256;
use anyhow::{anyhow, Context, Ok, Result};
use db::{BlockTraces, Database};
mod db;
mod rpc;
use rig::log::{debug, info};
use rig::Chain;

use crate::post_check::post_check;
use crate::prestate::populate_prestate;
use crate::{
    prestate::{DiffTrace, PrestateTrace},
    receipts::TransactionReceipt,
};

const N_PREV_BLOCKS: usize = 256;

// Fetches hashes for the N_PREV_BLOCKS previous to [start_block].
// Persists them in DB.
fn fetch_block_hashes(start_block: u64, db: &Database, endpoint: &str) -> Result<()> {
    let first = start_block.saturating_sub(N_PREV_BLOCKS as u64);
    for n in first..start_block {
        if db.get_block_hash(n)?.is_some() {
            debug!("Block hash for {} already in DB, skipping", n);
        } else {
            let hash = rpc::get_block_hash(endpoint, n)
                .context(format!("Failed to fetch block hash for {}", n))?;
            db.set_block_hash(n, U256::from_be_bytes(hash.0))?;
            debug!("Saved block hash for block {}: {:#x}", n, hash);
        }
    }
    Ok(())
}

// Constructs the array of previous N_PREV_BLOCKS block hashes from
// database.
fn get_block_hashes_array(block_number: u64, db: &Database) -> Result<[U256; N_PREV_BLOCKS]> {
    let mut hashes = [U256::ZERO; N_PREV_BLOCKS];
    // Add values for most recent blocks
    for offset in 1..=N_PREV_BLOCKS {
        if let Some(hash) = db.get_block_hash(block_number - (offset as u64))? {
            hashes[offset - 1] = U256::from(hash);
        } else {
            return Err(anyhow!(format!(
                "DB should have hash for block {}",
                block_number
            )));
        }
    }
    Ok(hashes)
}

// Does not persist the traces.
fn fetch_block_traces(block_number: u64, db: &Database, endpoint: &str) -> Result<BlockTraces> {
    match db.get_block_traces(block_number)? {
        Some(traces) => {
            debug!("Block traces for {} already in DB, skipping", block_number);
            Ok(traces)
        }
        None => {
            let block = rpc::get_block(endpoint, block_number)
                .context(format!("Failed to fetch block for {}", block_number))?;
            let prestate = rpc::get_prestate(endpoint, block_number).context(format!(
                "Failed to fetch prestate trace for {}",
                block_number
            ))?;
            let diff = rpc::get_difftrace(endpoint, block_number)
                .context(format!("Failed to fetch diff trace for {}", block_number))?;
            let receipts = rpc::get_receipts(endpoint, block_number).context(format!(
                "Failed to fetch block receipts for {}",
                block_number
            ))?;
            let block_traces = BlockTraces {
                block,
                prestate,
                diff,
                receipts,
            };
            Ok(block_traces)
        }
    }
}

fn run_block(block_number: u64, db: &Database, endpoint: &str) -> Result<()> {
    let block_traces = fetch_block_traces(block_number, db, endpoint)?;
    let traces_clone = block_traces.clone();

    let BlockTraces {
        prestate,
        diff,
        block,
        receipts,
    } = block_traces;
    // set block hash for future blocks to use
    db.set_block_hash(
        block_number,
        U256::from_be_bytes(block.result.header.hash.0),
    )?;
    info!("Running block: {}", block_number);
    info!("Block gas used: {}", block.result.header.gas_used);

    let miner = block.result.header.miner;
    let block_context = block.get_block_context();
    let (transactions, skipped) = block.get_transactions();
    let receipts: Vec<TransactionReceipt> = receipts
        .result
        .into_iter()
        .enumerate()
        .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
        .collect();

    let ps_trace = PrestateTrace {
        result: prestate
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    let diff_trace = DiffTrace {
        result: diff
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };
    let mut chain = Chain::empty_randomized(Some(1));
    chain.set_last_block_number(block_number - 1);

    chain.set_block_hashes(get_block_hashes_array(block_number, db)?);

    let prestate_cache = populate_prestate(&mut chain, ps_trace);

    let output = chain.run_block(transactions, Some(block_context), None);

    match post_check(
        output,
        receipts,
        diff_trace,
        prestate_cache,
        ruint::aliases::B160::from_be_bytes(miner.into()),
    ) {
        core::result::Result::Ok(()) => {
            db.set_block_status(block_number, db::BlockStatus::Success)?
        }
        Err(e) => {
            db.set_block_status(block_number, db::BlockStatus::Error(e))?;
            // Always save of them for now, even when already cached.
            // TODO: avoid persisting when read from cache.
            db.set_block_traces(block_number, &traces_clone)?;
            debug!("Saved block traces for block {}", block_number);
        }
    }

    Ok(())
}

///
/// Run blocks from [start_block] to [end_block].
///
pub fn live_run(start_block: u64, end_block: u64, endpoint: String, db_path: String) -> Result<()> {
    let db = Database::init(db_path)?;
    assert!(start_block <= end_block);
    fetch_block_hashes(start_block, &db, &endpoint)?;
    for n in start_block..=end_block {
        run_block(n, &db, &endpoint)?
    }
    Ok(())
}
