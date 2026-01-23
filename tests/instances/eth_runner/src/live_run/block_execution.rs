use super::db::{BlockStatus, BlockTraces, Database, ResourceInfo};
use super::utils;
use alloy::primitives::U256;
use anyhow::{anyhow, Result};
use rig::log::{debug, error, info, warn};
use rig::Chain;
use std::panic::AssertUnwindSafe;
use std::time::Instant;
use zk_ee::system::tracer::NopTracer;

use crate::calltrace::CallTrace;
use crate::native_model::compute_ratio;
use crate::post_check::post_check;
use crate::prestate::populate_prestate;
use crate::{
    prestate::{DiffTrace, PrestateTrace},
    receipts::TransactionReceipt,
};
use std::collections::HashSet;

/// Filters out items at indices that are in the skipped set.
///
/// Used to remove skipped transactions from receipts, traces, and other collections.
fn filter_skipped<T>(items: Vec<T>, skipped: &HashSet<usize>) -> Vec<T> {
    items.into_iter()
        .enumerate()
        .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
        .collect()
}

#[cfg(feature = "gpu")]
pub type GpuSharedState = rig::cli_lib::prover_utils::GpuSharedState;

#[cfg(all(feature = "proving", not(feature = "gpu")))]
pub type GpuSharedState<'a> = rig::cli_lib::prover_utils::GpuSharedState<'a>;

#[cfg(not(feature = "proving"))]
pub type GpuSharedState = ();

/// Runs a block using prefetched traces.
#[allow(clippy::too_many_arguments, unused_variables)]
pub fn run_block(
    block_number: u64,
    db: &Database,
    endpoint: &str,
    witness_output_dir: Option<String>,
    persist_all: bool,
    chain_id: u64,
    single_tx: Option<u64>,
    gpu_shared_state: &mut Option<&mut GpuSharedState>,
    only_forward: bool,
    block_traces: BlockTraces,
) -> Result<BlockStatus> {
    let block_start = Instant::now();
    let traces_clone = block_traces.clone();

    let BlockTraces {
        prestate,
        diff,
        block,
        receipts,
        call,
    } = block_traces;
    
    info!("\n ===================");
    info!("Running block: {block_number}");

    // Extract block hash before block is moved by get_transactions()
    let block_hash = U256::from_be_bytes(block.result.header.hash.0);
    
    let block_context = block.get_block_context();
    let (transactions, skipped, calls_unsupported_precompile) =
        block.get_transactions(&call, single_tx);
    if calls_unsupported_precompile {
        // Here it makes little sense to run the block, as the post check is gonna fail
        // We just skip it, marking it as successful
        // Set and flush block hash before returning so future blocks can reference it
        db.set_block_hash(block_number, block_hash)?;
        db.flush()?;
        warn!("Skipping block {block_number}, as it calls to an unsupported precompile");
        return Ok(BlockStatus::Success);
    }
    
    // Set block hash for future blocks to use
    db.set_block_hash(block_number, block_hash)?;
    info!("Transactions to run: {}", transactions.len());

    let receipts: Vec<TransactionReceipt> = filter_skipped(receipts.result, &skipped);

    let total_gas_used = receipts
        .iter()
        .fold(U256::ZERO, |acc, r| r.gas_used.wrapping_add(acc));
    info!("Reference gas used: {total_gas_used}");

    let ps_trace = PrestateTrace {
        result: filter_skipped(prestate.result, &skipped),
    };

    let diff_trace = DiffTrace {
        result: filter_skipped(diff.result, &skipped),
    };

    let calltrace = CallTrace {
        result: filter_skipped(call.result, &skipped),
    };

    let setup_start = Instant::now();
    let mut chain = Chain::empty_randomized(Some(chain_id));
    chain.set_last_block_number(block_number - 1);

    let db_hash_start = Instant::now();
    chain.set_block_hashes(utils::get_block_hashes_array(block_number, db)?);
    let db_hash_time = db_hash_start.elapsed();

    let prestate_start = Instant::now();
    let prestate_cache = populate_prestate(&mut chain, ps_trace, &calltrace);
    let prestate_time = prestate_start.elapsed();
    let setup_time = setup_start.elapsed();

    let output_path = witness_output_dir.map(|dir| {
        let mut suffix = block_number.to_string();
        suffix.push_str("_witness");
        std::path::Path::new(&dir).join(suffix)
    });
    
    let run_config = rig::chain::RunConfig {
        witness_output_file: output_path,
        only_forward,
        app: Some("evm_replay".to_string()),
        check_storage_diff_hashes: true,
        ..Default::default()
    };
    
    let execution_start = Instant::now();
    
    // Wrap execution in panic handler to catch panics
    let execution_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        chain.run_block_with_extra_stats(
            transactions,
            Some(block_context),
            None,
            Some(run_config),
            &mut NopTracer::default(),
        )
    }));
    
    let (output, stats, _prover_input) = match execution_result {
        std::result::Result::Ok(std::result::Result::Ok(result)) => result,
        std::result::Result::Ok(std::result::Result::Err(e)) => {
            return Err(anyhow!("Block execution failed: {e:?}"));
        }
        std::result::Result::Err(panic_payload) => {
            // Extract panic message if possible
            let panic_msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                format!("Panic occurred (payload type: {:?})", panic_payload.type_id())
            };
            
            error!("Block {block_number} panicked during execution: {panic_msg}");
            return Err(anyhow!("Block {block_number} panicked during execution: {panic_msg}"));
        }
    };
    
    let execution_time = execution_start.elapsed();

    info!("Actual gas used: {}", output.header.gas_used);

    #[cfg(feature = "proving")]
    {
        let bin_path = rig::chain::get_zksync_os_img_path(&Some("evm_replay".to_string()))
            .as_path()
            .to_str()
            .unwrap()
            .to_string();
        let witness: Vec<u8> = _prover_input.iter().flat_map(|x| x.to_be_bytes()).collect();
        let input_hex = hex::encode(witness);
        let non_determinism_data = rig::cli_lib::prover_utils::u32_from_hex_string(&input_hex);
        let binary = rig::cli_lib::prover_utils::load_binary_from_path(&bin_path);
        #[cfg(not(feature = "gpu"))]
        let gpu_shared_state = &mut None;
        let mut total_proof_time = Some(0f64);

        info!("Starting base layer proofs...");
        rig::cli_lib::prover_utils::create_proofs_internal(
            &binary,
            non_determinism_data,
            &rig::cli_lib::Machine::Standard,
            1024,
            None,
            gpu_shared_state,
            &mut total_proof_time,
        );
        info!("Done with base layer proofs");
    }

    let db_write_start = Instant::now();
    if let Some(ratio) = compute_ratio(stats) {
        db.set_block_ratio(block_number, ratio)?;
    }

    let resource_infos: Vec<ResourceInfo> = output
        .tx_results
        .iter()
        .filter_map(|r| {
            r.as_ref().ok().map(|r| ResourceInfo::V0 {
                native_used: r.native_used,
                computational_native_used: r.computational_native_used,
                gas_used: r.gas_used,
                pubdata_used: r.pubdata_used,
                logs_used: r.logs.len() as u64,
            })
        })
        .collect();

    db.set_block_resource_infos(block_number, resource_infos)?;
    
    // Flush once after all writes are batched
    let flush_start = Instant::now();
    db.flush()?;
    let flush_time = flush_start.elapsed();
    let db_write_time = db_write_start.elapsed();

    let post_check_start = Instant::now();
    let post_check_result = post_check(output, receipts, diff_trace, prestate_cache);
    let post_check_time = post_check_start.elapsed();
    
    let total_time = block_start.elapsed();
    
    // Log timing breakdown
    info!("=== Block {} Timing Breakdown ===", block_number);
    // Fetch time is 0 since traces are prefetched
    let fetch_time = std::time::Duration::ZERO;
    info!("  Fetch traces:     {:6.2}ms ({:5.1}%)", fetch_time.as_secs_f64() * 1000.0, fetch_time.as_secs_f64() / total_time.as_secs_f64() * 100.0);
    info!("  Setup:             {:6.2}ms ({:5.1}%)", setup_time.as_secs_f64() * 1000.0, setup_time.as_secs_f64() / total_time.as_secs_f64() * 100.0);
    info!("    - DB hash read: {:6.2}ms", db_hash_time.as_secs_f64() * 1000.0);
    info!("    - Prestate:     {:6.2}ms", prestate_time.as_secs_f64() * 1000.0);
    info!("  Execution:         {:6.2}ms ({:5.1}%)", execution_time.as_secs_f64() * 1000.0, execution_time.as_secs_f64() / total_time.as_secs_f64() * 100.0);
    info!("  Post-check:        {:6.2}ms ({:5.1}%)", post_check_time.as_secs_f64() * 1000.0, post_check_time.as_secs_f64() / total_time.as_secs_f64() * 100.0);
    info!("  DB writes:         {:6.2}ms ({:5.1}%)", db_write_time.as_secs_f64() * 1000.0, db_write_time.as_secs_f64() / total_time.as_secs_f64() * 100.0);
    info!("    - Flush:         {:6.2}ms ({:5.1}% of DB writes)", flush_time.as_secs_f64() * 1000.0, flush_time.as_secs_f64() / db_write_time.as_secs_f64() * 100.0);
    info!("  Total:             {:6.2}ms", total_time.as_secs_f64() * 1000.0);
    info!("===================================");

    match post_check_result {
        core::result::Result::Ok(()) => {
            let post_db_write_start = Instant::now();
            db.set_block_status(block_number, BlockStatus::Success)?;
            if persist_all {
                db.set_block_traces(block_number, &traces_clone)?;
            }
            // Flush status and traces writes
            let post_flush_start = Instant::now();
            db.flush()?;
            let post_flush_time = post_flush_start.elapsed();
            let post_db_write_time = post_db_write_start.elapsed();
            debug!("Post-check DB writes: {:.2}ms (flush: {:.2}ms)", 
                post_db_write_time.as_secs_f64() * 1000.0,
                post_flush_time.as_secs_f64() * 1000.0
            );
            Ok(BlockStatus::Success)
        }
        Err(e) => {
            let post_db_write_start = Instant::now();
            db.set_block_status(block_number, BlockStatus::Error(e.clone()))?;
            // Always save of them for now, even when already cached.
            // TODO: avoid persisting when read from cache.
            db.set_block_traces(block_number, &traces_clone)?;
            
            // Flush status and traces writes
            let post_flush_start = Instant::now();
            db.flush()?;
            let post_flush_time = post_flush_start.elapsed();
            let post_db_write_time = post_db_write_start.elapsed();
            debug!("Post-check DB writes: {:.2}ms (flush: {:.2}ms)", 
                post_db_write_time.as_secs_f64() * 1000.0,
                post_flush_time.as_secs_f64() * 1000.0
            );
            debug!("Saved block traces for block {block_number}");
            Ok(BlockStatus::Error(e))
        }
    }
}
