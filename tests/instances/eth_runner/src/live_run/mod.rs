use anyhow::Result;
mod block_execution;
mod db;
mod error_handling;
mod prefetch;
mod rpc;
mod statistics;
mod utils;
use block_execution::GpuSharedState;
use db::{BlockStatus, BlockTraces, Database};
use rig::log::{debug, info, warn};
use statistics::RunStatistics;
use std::sync::atomic::Ordering;
use std::time::Instant;

pub fn live_run(
    start_block: u64,
    end_block: u64,
    endpoint: String,
    db_path: String,
    witness_output_dir: Option<String>,
    skip_successful: bool,
    persist_all: bool,
    webhook: Option<String>,
    single_tx: Option<u64>,
    only_forward: bool,
    backup_endpoint: Option<String>,
) -> Result<()> {
    let run_start = Instant::now();

    // Install panic hook (with or without webhook)
    utils::install_panic_hook(webhook.clone());

    let init_start = Instant::now();
    let db = Database::init(db_path)?;
    assert!(start_block <= end_block);
    utils::fetch_block_hashes(start_block, &db, &endpoint)?;
    let chain_id = rpc::get_chain_id(&endpoint)?;
    let init_time = init_start.elapsed();

    info!("=== Live Run Started ===");
    info!("Blocks: {} to {}", start_block, end_block);
    info!("Initialization: {:.2}ms", init_time.as_secs_f64() * 1000.0);

    #[cfg(feature = "gpu")]
    let mut gpu_state = {
        info!("Setting up GPU state...");
        let bin_path = rig::chain::get_zksync_os_img_path(&Some("evm_replay".to_string()))
            .as_path()
            .to_str()
            .unwrap()
            .to_string();
        let binary = rig::cli_lib::prover_utils::load_binary_from_path(&bin_path);
        let s = rig::cli_lib::prover_utils::GpuSharedState::new(
            &binary,
            rig::gpu_prover::circuit_type::MainCircuitType::ReducedRiscVMachine,
        );
        info!("Done setting up GPU state...");
        s
    };
    #[cfg(feature = "gpu")]
    let gpu_state = &mut Some(&mut gpu_state);

    #[cfg(not(feature = "gpu"))]
    let gpu_state: &mut Option<&mut GpuSharedState> = &mut None;

    let mut stats = RunStatistics::new();
    let mut prefetch_cache = std::collections::HashMap::<u64, BlockTraces>::new();
    let mut next_block_to_prefetch = start_block;
    let mut stopped_early = false;

    for n in start_block..=end_block {
        // Update current block number for panic handler
        utils::CURRENT_BLOCK_NUMBER.store(n, Ordering::Relaxed);

        // Prefetch next batch if cache is empty
        prefetch::prefetch_next_batch(
            &mut next_block_to_prefetch,
            end_block,
            &db,
            &endpoint,
            &mut prefetch_cache,
            &mut stats.total_prefetch_time,
            &mut stats.total_blocks_prefetched,
            skip_successful,
        )?;

        // Check if we should skip this block
        // Remove from prefetch cache first to prevent cache from stalling
        if let std::result::Result::Ok(Some(status)) = db.get_block_status(n) {
            if skip_successful && matches!(status, BlockStatus::Success) {
                // Remove from cache before skipping to prevent prefetch stall
                prefetch_cache.remove(&n);
                debug!("Skipping block {n}, already succeeded");
                stats.blocks_skipped_already_succeeded += 1;
                continue;
            }
        }

        // Get traces from prefetch cache or fetch if not available
        let block_traces = if let Some(traces) = prefetch_cache.remove(&n) {
            stats.prefetch_hits += 1;
            traces
        } else {
            stats.prefetch_misses += 1;
            match error_handling::fetch_block_traces_with_backup(
                n,
                &db,
                &endpoint,
                backup_endpoint.as_ref(),
                chain_id,
                webhook.as_ref(),
                &mut stats,
            )? {
                Some(traces) => traces,
                None => continue, // Block skipped due to trace fetch failure
            }
        };

        // Process block sequentially
        let block_start = Instant::now();
        let primary_result = block_execution::run_block(
            n,
            &db,
            &endpoint,
            witness_output_dir.clone(),
            persist_all,
            chain_id,
            single_tx,
            gpu_state,
            only_forward,
            block_traces,
        );
        let block_time = block_start.elapsed();
        stats.total_block_time += block_time;

        // Retry block execution with backup endpoint if primary execution failed
        let result = match primary_result {
            Ok(BlockStatus::Success) => Ok(BlockStatus::Success),
            failed_result => {
                if let Some(backup) = backup_endpoint.as_ref() {
                    error_handling::retry_block_with_backup_endpoint(
                        n,
                        backup,
                        &db,
                        witness_output_dir.clone(),
                        persist_all,
                        chain_id,
                        single_tx,
                        gpu_state,
                        only_forward,
                        &mut stats.total_block_time,
                    )
                } else {
                    failed_result
                }
            }
        };

        // Handle result (update stats, send webhooks, check failures)
        // If max failures reached, break out of loop gracefully
        if let Err(e) =
            error_handling::handle_block_result(result, n, chain_id, webhook.as_ref(), &mut stats)
        {
            warn!("Stopping execution: {e}");
            stopped_early = true;
            break;
        }
    }

    let total_time = run_start.elapsed();
    statistics::log_run_statistics(
        start_block,
        end_block,
        chain_id,
        init_time,
        total_time,
        &stats,
    );

    if let Some(webhook) = webhook.as_ref() {
        let machine_info = utils::get_machine_info();
        let (emoji, status_msg) = if stopped_early {
            (":rotating_light:", "stopped early due to max failures")
        } else {
            (":white_check_mark:", "successfully!")
        };
        let msg = format!(
            "{emoji} eth_runner: finished running from block {start_block} to {end_block} on chain with id {chain_id} {status_msg}\n\
            \n\
            *Block Range:* {start_block} to {end_block}\n\
            *Chain ID:* {chain_id}\n\
            *Blocks Processed:* {}\n\
            *Blocks Skipped (already succeeded):* {}\n\
            *Blocks Skipped (trace fetch failed):* {}\n\
            *Failures:* {} ({} critical)\n\
            \n\
            *Machine Info:*\n\
            {machine_info}",
            stats.blocks_actually_processed,
            stats.blocks_skipped_already_succeeded,
            stats.blocks_skipped_trace_fetch,
            stats.failures,
            stats.critical_failures
        );
        utils::send_slack(webhook, &msg)?
    }
    Ok(())
}

///
/// Export native/effective cycles ratios to csv file.
///
pub fn export_block_ratios(db: String, path: Option<String>) -> Result<()> {
    let db = Database::init(db)?;
    let path = path.unwrap_or("ratios.csv".to_string());
    db.export_block_ratios_to_csv(&path)?;
    db.export_block_resource_info_to_csv("resource_info.csv")?;
    Ok(())
}

///
/// Show failed blocks, if any.
///
pub fn show_status(db: String) -> Result<()> {
    let db = Database::init(db)?;
    let failures = db.iter_failed_block_statuses()?;
    if failures.is_empty() {
        println!("✅ All blocks succeeded.");
        Ok(())
    } else {
        println!("❌ Failed blocks:");
        for (block_number, status) in failures {
            println!("Block {block_number:<8} => {status:?}");
        }
        Ok(())
    }
}
