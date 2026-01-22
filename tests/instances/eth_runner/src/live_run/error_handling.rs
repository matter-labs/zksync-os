use super::block_execution::{run_block, GpuSharedState};
use super::db::{BlockStatus, BlockTraces, Database};
use super::prefetch::fetch_block_traces;
use super::rpc;
use super::statistics::RunStatistics;
use super::utils;
use alloy::primitives::U256;
use anyhow::{anyhow, Context, Result};
use rig::log::{debug, error, info, warn};
use std::time::Instant;

use crate::post_check::PostCheckError;

const MAX_FAILURES: usize = 10;

/// Formats a block failure message for Slack webhook notifications.
fn format_block_failure_message(block_number: u64, chain_id: u64, error_type: &str, error: &dyn std::fmt::Debug) -> String {
    let machine_info = utils::get_machine_info();
    format!(
        ":rotating_light: eth_runner: Block {block_number} on chain with id {chain_id} {error_type}\n\
        \n\
        *Block Number:* {block_number}\n\
        *Chain ID:* {chain_id}\n\
        \n\
        *Machine Info:*\n\
        {machine_info}\n\
        \n\
        *Error:*\n\
        {error:?}"
    )
}

/// Retries block execution using a backup endpoint after primary execution failed.
///
/// Fetches traces from backup endpoint and retries block execution.
/// Updates total_block_time with backup execution time.
///
/// Note: This function should only be called when primary execution has already failed.
pub fn retry_block_with_backup_endpoint(
    block_number: u64,
    backup_endpoint: &str,
    db: &Database,
    witness_output_dir: Option<String>,
    persist_all: bool,
    chain_id: u64,
    single_tx: Option<u64>,
    gpu_state: &mut Option<&mut GpuSharedState>,
    only_forward: bool,
    total_block_time: &mut std::time::Duration,
) -> Result<BlockStatus> {
    warn!("Block {block_number} failed with primary endpoint. Retrying with backup endpoint...");
    
    let backup_traces_result = {
        let rpc_start = Instant::now();
        match rpc::get_all_block_traces(backup_endpoint, block_number)
            .context(format!("Failed to fetch block traces from backup endpoint for {block_number}"))
        {
            std::result::Result::Ok((block, prestate, diff, receipts, call)) => {
                let total_rpc_time = rpc_start.elapsed();
                debug!("RPC call for block {} from backup endpoint (batched): total={:.2}ms",
                    block_number,
                    total_rpc_time.as_secs_f64() * 1000.0
                );
                std::result::Result::Ok(BlockTraces {
                    block,
                    prestate,
                    diff,
                    receipts,
                    call,
                })
            }
            std::result::Result::Err(e) => std::result::Result::Err(e),
        }
    };
    
    match backup_traces_result {
        std::result::Result::Ok(backup_traces) => {
            let backup_block_start = Instant::now();
            let backup_result = run_block(
                block_number,
                db,
                backup_endpoint,
                witness_output_dir,
                persist_all,
                chain_id,
                single_tx,
                gpu_state,
                only_forward,
                backup_traces,
            );
            let backup_block_time = backup_block_start.elapsed();
            *total_block_time += backup_block_time;
            
            match backup_result {
                std::result::Result::Ok(BlockStatus::Success) => {
                    info!("Block {block_number} succeeded with backup endpoint");
                    std::result::Result::Ok(BlockStatus::Success)
                }
                std::result::Result::Ok(BlockStatus::Error(backup_e)) => {
                    warn!("Block {block_number} also failed with backup endpoint: {backup_e:?}");
                    std::result::Result::Ok(BlockStatus::Error(backup_e))
                }
                std::result::Result::Err(backup_e) => {
                    warn!("Block {block_number} also failed with backup endpoint: {backup_e:?}");
                    std::result::Result::Err(backup_e)
                }
            }
        }
        std::result::Result::Err(fetch_err) => {
            error!("Failed to fetch traces from backup endpoint for block {block_number}: {fetch_err:?}");
            Err(anyhow!("Backup endpoint failed to fetch traces: {fetch_err:?}")
                .context(format!("Block {} failed with primary endpoint and backup fetch also failed", block_number)))
        }
    }
}

/// Fetches block traces with automatic fallback to backup endpoint.
///
/// Tries primary endpoint first, then backup if available. If both fail, sends webhook notification
/// and attempts to save at least the block hash for future blocks.
pub fn fetch_block_traces_with_backup(
    block_number: u64,
    db: &Database,
    primary_endpoint: &str,
    backup_endpoint: Option<&String>,
    chain_id: u64,
    webhook: Option<&String>,
    stats: &mut RunStatistics,
) -> Result<Option<BlockTraces>> {
    // Try primary endpoint first
    match fetch_block_traces(block_number, db, primary_endpoint) {
        std::result::Result::Ok(traces) => Ok(Some(traces)),
        std::result::Result::Err(primary_err) => {
            error!("Failed to fetch traces for block {block_number} from primary endpoint: {primary_err:?}");
            
            // Try backup endpoint if available
            let traces_result = if let Some(backup) = backup_endpoint {
                warn!("Trying backup endpoint for block {block_number} trace fetch...");
                match rpc::get_all_block_traces(backup, block_number)
                    .context(format!("Failed to fetch block traces from backup endpoint for {block_number}"))
                {
                    std::result::Result::Ok((block, prestate, diff, receipts, call)) => {
                        info!("Successfully fetched traces for block {block_number} from backup endpoint");
                        std::result::Result::Ok(BlockTraces {
                            block,
                            prestate,
                            diff,
                            receipts,
                            call,
                        })
                    }
                    std::result::Result::Err(backup_err) => {
                        error!("Failed to fetch traces for block {block_number} from backup endpoint: {backup_err:?}");
                        std::result::Result::Err(backup_err)
                    }
                }
            } else {
                std::result::Result::Err(primary_err)
            };
            
            match traces_result {
                std::result::Result::Ok(traces) => Ok(Some(traces)),
                std::result::Result::Err(e) => {
                    // Both endpoints failed - send webhook notification and skip block
                    stats.blocks_skipped_trace_fetch += 1;
                    if let Some(webhook) = webhook {
                        let machine_info = utils::get_machine_info();
                        let msg = format!(
                            ":rotating_light: eth_runner: Failed to fetch traces for block {block_number} on chain with id {chain_id}\n\
                            \n\
                            *Block Number:* {block_number}\n\
                            *Chain ID:* {chain_id}\n\
                            \n\
                            *Machine Info:*\n\
                            {machine_info}\n\
                            \n\
                            *Error:*\n\
                            {e:?}"
                        );
                        if let Err(webhook_err) = utils::send_slack(webhook, &msg) {
                            warn!("Failed to send webhook notification: {}", webhook_err);
                        }
                    }
                    
                    // Even if we can't fetch traces, we need to save the block hash
                    // so future blocks can reference it. Try to fetch just the hash.
                    match db.get_block_hash(block_number) {
                        std::result::Result::Ok(Some(_)) => {
                            // Hash already exists, nothing to do
                        }
                        std::result::Result::Ok(None) => {
                            // Hash doesn't exist, try to fetch it
                            // Try backup endpoint first if available (since primary already failed for traces)
                            let hash_result = if let Some(backup) = backup_endpoint {
                                rpc::get_block_hash(backup, block_number)
                                    .or_else(|_| rpc::get_block_hash(primary_endpoint, block_number))
                            } else {
                                rpc::get_block_hash(primary_endpoint, block_number)
                            };
                            
                            match hash_result {
                                std::result::Result::Ok(hash) => {
                                    if let Err(hash_err) = db.set_block_hash(block_number, U256::from_be_bytes(hash.0)) {
                                        warn!("Failed to save block hash for {block_number}: {hash_err}");
                                    } else {
                                        if let Err(flush_err) = db.flush() {
                                            warn!("Failed to flush DB after saving block hash for {block_number}: {flush_err}");
                                        }
                                        debug!("Saved block hash for block {block_number}");
                                    }
                                }
                                std::result::Result::Err(hash_err) => {
                                    warn!("Failed to fetch block hash for {block_number} from both endpoints: {hash_err}");
                                }
                            }
                        }
                        std::result::Result::Err(_) => {
                            // If get_block_hash returns an error, we just skip saving the hash
                        }
                    }
                    Ok(None) // Return None to indicate block should be skipped
                }
            }
        }
    }
}

/// Handles block execution result, updating statistics and sending notifications.
///
/// Updates success/failure counters, sends webhook notifications for failures, and checks
/// if max critical failures threshold is reached (stops execution if exceeded).
pub fn handle_block_result(
    result: Result<BlockStatus>,
    block_number: u64,
    chain_id: u64,
    webhook: Option<&String>,
    stats: &mut RunStatistics,
) -> Result<()> {
    match result {
        std::result::Result::Ok(BlockStatus::Success) => {
            stats.blocks_actually_processed += 1;
        }
        std::result::Result::Ok(BlockStatus::Error(e)) => {
            stats.failures += 1;
            
            // Check if this is a "Reference must have write for account" error
            let should_skip_webhook = if let PostCheckError::Internal { msg } = &e {
                msg.contains("Reference must have write for account")
            } else {
                false
            };
            
            if should_skip_webhook {
                warn!("Block {block_number} failed with 'Reference must have write for account' error: {e:?}");
                // Don't count this towards critical failures (MAX_FAILURES check)
            } else {
                stats.critical_failures += 1;
                let webhook_start = Instant::now();
                if let Some(webhook) = webhook {
                    let msg = format_block_failure_message(block_number, chain_id, "failed", &e);
                    utils::send_slack(webhook, &msg)?;
                }
                stats.total_overhead_time += webhook_start.elapsed();
            }
            
            if stats.critical_failures == MAX_FAILURES {
                error!("Reached max number of critical failures ({MAX_FAILURES}), stopping execution");
                return Err(anyhow!("Reached max number of critical failures ({MAX_FAILURES})"));
            }
        }
        std::result::Result::Err(e) => {
            stats.failures += 1;
            stats.critical_failures += 1;
            error!("Block {block_number} failed with error: {e:?}");
            let webhook_start = Instant::now();
            if let Some(webhook) = webhook {
                let msg = format_block_failure_message(block_number, chain_id, "failed with execution error", &e);
                utils::send_slack(webhook, &msg)?;
            }
            stats.total_overhead_time += webhook_start.elapsed();
            
            if stats.critical_failures == MAX_FAILURES {
                error!("Reached max number of critical failures ({MAX_FAILURES}), stopping execution");
                return Err(anyhow!("Reached max number of critical failures ({MAX_FAILURES})"));
            }
        }
    }
    Ok(())
}
