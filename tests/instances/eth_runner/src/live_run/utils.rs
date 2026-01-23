use super::db::Database;
use alloy::primitives::U256;
use anyhow::{anyhow, Context, Result};
use rig::log::{debug, info};
use reqwest::blocking::Client;
use std::backtrace::Backtrace;
use std::panic;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// Global variable to track current block number for panic handler
pub(crate) static CURRENT_BLOCK_NUMBER: AtomicU64 = AtomicU64::new(0);

const N_PREV_BLOCKS: usize = 256;

/// Sends a message to Slack via webhook.
pub fn send_slack(webhook: &str, text: &str) -> Result<()> {
    let resp = Client::new()
        .post(webhook)
        .json(&serde_json::json!({ "text": text }))
        .send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("slack webhook returned {}", resp.status()));
    }
    Ok(())
}

/// Collects machine information for logging and notifications.
///
/// Returns hostname, process number, and PID from environment variables and system.
pub fn get_machine_info() -> String {
    let mut info = Vec::new();
    
    // Hostname from environment variable
    if let std::result::Result::Ok(hostname) = std::env::var("HOSTNAME") {
        info.push(format!("Hostname: {}", hostname));
    }
    
    // Process number from environment variable
    if let std::result::Result::Ok(proc_num) = std::env::var("PROC_NUM") {
        info.push(format!("Process Number: {}", proc_num));
    }
    
    // Process info
    info.push(format!("PID: {}", std::process::id()));
    
    info.join("\n")
}

/// Installs a panic hook that logs panic information and optionally sends to Slack.
///
/// Captures current block number, machine info, panic message, and backtrace.
/// Always logs to stderr, sends to Slack only if webhook is provided.
pub fn install_panic_hook(webhook: Option<String>) {
    panic::set_hook(Box::new(move |info| {
        let current_block = CURRENT_BLOCK_NUMBER.load(Ordering::Relaxed);
        let machine_info = get_machine_info();
        let backtrace = Backtrace::force_capture();
        
        let msg = format!(
            ":rotating_light: eth-runner panicked\n\
            \n\
            *Block Number:* {}\n\
            \n\
            *Machine Info:*\n\
            {}\n\
            \n\
            *Panic Info:*\n\
            {}\n\
            \n\
            *Backtrace:*\n\
            {}",
            if current_block == 0 { "Unknown".to_string() } else { current_block.to_string() },
            machine_info,
            info,
            backtrace
        );
        
        // Always print to stderr
        info!("{msg}");
        
        // Only send to Slack if webhook is provided
        if let Some(webhook_url) = &webhook {
            let _ = send_slack(webhook_url, &msg);
        }
    }));
}

/// Fetches block hashes for the N_PREV_BLOCKS previous to start_block.
///
/// Persists them in DB. Uses batched RPC call to fetch all missing hashes in a single request.
pub fn fetch_block_hashes(start_block: u64, db: &Database, endpoint: &str) -> Result<()> {
    use super::rpc;
    
    let first = start_block.saturating_sub(N_PREV_BLOCKS as u64);
    
    // Collect all block numbers that need to be fetched
    let mut blocks_to_fetch = Vec::new();
    for n in first..start_block {
        if db.get_block_hash(n)?.is_none() {
            blocks_to_fetch.push(n);
        } else {
            debug!("Block hash for {n} already in DB, skipping");
        }
    }
    
    if blocks_to_fetch.is_empty() {
        debug!("All block hashes already in DB, skipping fetch");
        return Ok(());
    }
    
    debug!("Fetching {} block hashes in batched RPC call", blocks_to_fetch.len());
    
    // Fetch all missing hashes in a single batched RPC call
    let hashes = rpc::get_block_hashes_batch(endpoint, &blocks_to_fetch)
        .context(format!("Failed to fetch block hashes in batch"))?;
    
    // Save all hashes to DB
    let blocks_count = blocks_to_fetch.len();
    for block_num in blocks_to_fetch {
        if let Some(hash) = hashes.get(&block_num) {
            db.set_block_hash(block_num, U256::from_be_bytes(hash.0))?;
            debug!("Saved block hash for block {block_num}: {hash:#x}");
        } else {
            return Err(anyhow!("Missing hash for block {block_num} in batched response"));
        }
    }
    
    // Flush all block hash writes after batching
    let flush_start = Instant::now();
    db.flush()?;
    let flush_time = flush_start.elapsed();
    debug!("Flushed {} block hashes in {:.2}ms", blocks_count, flush_time.as_secs_f64() * 1000.0);
    
    Ok(())
}

/// Constructs an array of the previous N_PREV_BLOCKS block hashes from database.
///
/// Returns hashes for blocks [block_number - N_PREV_BLOCKS, block_number - 1].
/// Fails if any required hash is missing from the database.
pub fn get_block_hashes_array(block_number: u64, db: &Database) -> Result<[U256; N_PREV_BLOCKS]> {
    let mut hashes = [U256::ZERO; N_PREV_BLOCKS];
    // Add values for most recent blocks
    for offset in 1..=N_PREV_BLOCKS {
        if let Some(hash) = db.get_block_hash(block_number - (offset as u64))? {
            hashes[N_PREV_BLOCKS - offset] = U256::from(hash);
        } else {
            return Err(anyhow!(format!(
                "DB should have hash for block {}",
                block_number
            )));
        }
    }
    Ok(hashes)
}

