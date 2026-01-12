use alloy::primitives::U256;
use anyhow::{anyhow, Context, Ok, Result};
use db::{BlockStatus, BlockTraces, Database, ResourceInfo};
mod db;
mod rpc;
use rig::log::{debug, error, info, warn};
use rig::Chain;
use zk_ee::system::tracer::NopTracer;

use crate::calltrace::CallTrace;
use crate::native_model::compute_ratio;
use crate::post_check::post_check;
use crate::prestate::populate_prestate;
use crate::{
    prestate::{DiffTrace, PrestateTrace},
    receipts::TransactionReceipt,
};
use reqwest::blocking::Client;
use serde_json::json;
use std::backtrace::Backtrace;
use std::panic;

const N_PREV_BLOCKS: usize = 256;
const MAX_FAILURES: usize = 10;

fn send_slack(webhook: &str, text: &str) -> Result<()> {
    let resp = Client::new()
        .post(webhook)
        .json(&serde_json::json!({ "text": text }))
        .send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("slack webhook returned {}", resp.status()));
    }
    Ok(())
}

fn install_panic_hook(webhook: String) {
    panic::set_hook(Box::new(move |info| {
        let msg = format!(
            ":rotating_light: eth-runner panicked: {info}\n{}",
            Backtrace::force_capture()
        );
        let _ = Client::new()
            .post(&webhook)
            .json(&json!({ "text": msg }))
            .send();
        eprintln!("{msg}");
    }));
}

// Fetches hashes for the N_PREV_BLOCKS previous to [start_block].
// Persists them in DB.
fn fetch_block_hashes(start_block: u64, db: &Database, endpoint: &str) -> Result<()> {
    let first = start_block.saturating_sub(N_PREV_BLOCKS as u64);
    for n in first..start_block {
        if db.get_block_hash(n)?.is_some() {
            debug!("Block hash for {n} already in DB, skipping");
        } else {
            let hash = rpc::get_block_hash(endpoint, n)
                .context(format!("Failed to fetch block hash for {n}"))?;
            db.set_block_hash(n, U256::from_be_bytes(hash.0))?;
            debug!("Saved block hash for block {n}: {hash:#x}");
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

// Does not persist the traces.
fn fetch_block_traces(block_number: u64, db: &Database, endpoint: &str) -> Result<BlockTraces> {
    match db.get_block_traces(block_number)? {
        Some(traces) => {
            debug!("Block traces for {block_number} already in DB, skipping");
            Ok(traces)
        }
        None => {
            let block = rpc::get_block(endpoint, block_number)
                .context(format!("Failed to fetch block for {block_number}"))?;
            let prestate = rpc::get_prestate(endpoint, block_number)
                .context(format!("Failed to fetch prestate trace for {block_number}"))?;
            let diff = rpc::get_difftrace(endpoint, block_number)
                .context(format!("Failed to fetch diff trace for {block_number}"))?;
            let receipts = rpc::get_receipts(endpoint, block_number)
                .context(format!("Failed to fetch block receipts for {block_number}"))?;
            let call = rpc::get_calltrace(endpoint, block_number)
                .context(format!("Failed to fetch call trace for {block_number}"))?;
            let block_traces = BlockTraces {
                block,
                prestate,
                diff,
                receipts,
                call,
            };
            Ok(block_traces)
        }
    }
}

#[cfg(feature = "gpu")]
type GpuSharedState = rig::cli_lib::prover_utils::GpuSharedState;

#[cfg(all(feature = "proving", not(feature = "gpu")))]
type GpuSharedState<'a> = rig::cli_lib::prover_utils::GpuSharedState<'a>;

#[cfg(not(feature = "proving"))]
type GpuSharedState = ();

#[allow(clippy::too_many_arguments, unused_variables)]
fn run_block(
    block_number: u64,
    db: &Database,
    endpoint: &str,
    witness_output_dir: Option<String>,
    persist_all: bool,
    chain_id: u64,
    single_tx: Option<u64>,
    gpu_shared_state: &mut Option<&mut GpuSharedState>,
    only_forward: bool,
) -> Result<BlockStatus> {
    let block_traces = fetch_block_traces(block_number, db, endpoint)?;
    let traces_clone = block_traces.clone();

    let BlockTraces {
        prestate,
        diff,
        block,
        receipts,
        call,
    } = block_traces;
    // set block hash for future blocks to use
    db.set_block_hash(
        block_number,
        U256::from_be_bytes(block.result.header.hash.0),
    )?;
    info!("\n ===================");
    info!("Running block: {block_number}");

    let block_context = block.get_block_context();
    let (transactions, skipped, calls_unsupported_precompile) =
        block.get_transactions(&call, single_tx);
    if calls_unsupported_precompile {
        // Here it makes little sense to run the block, as the post check is gonna fail
        // We just skip it, marking it as successful
        warn!("Skipping block {block_number}, as it calls to an unsupported precompile");
        return Ok(BlockStatus::Success);
    }
    info!("Transactions to run: {}", transactions.len());

    let receipts: Vec<TransactionReceipt> = receipts
        .result
        .into_iter()
        .enumerate()
        .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
        .collect();

    let total_gas_used = receipts
        .iter()
        .fold(U256::ZERO, |acc, r| r.gas_used.wrapping_add(acc));
    info!("Reference gas used: {total_gas_used}");

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

    let calltrace = CallTrace {
        result: call
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    let mut chain = Chain::empty_randomized(Some(chain_id));
    chain.set_last_block_number(block_number - 1);

    chain.set_block_hashes(get_block_hashes_array(block_number, db)?);

    let prestate_cache = populate_prestate(&mut chain, ps_trace, &calltrace);

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
    let (output, stats, _prover_input) = chain
        .run_block_with_extra_stats(
            transactions,
            Some(block_context),
            None,
            Some(run_config),
            &mut NopTracer::default(),
        )
        .unwrap();

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

    match post_check(output, receipts, diff_trace, prestate_cache) {
        core::result::Result::Ok(()) => {
            db.set_block_status(block_number, db::BlockStatus::Success)?;
            if persist_all {
                db.set_block_traces(block_number, &traces_clone)?;
            }
            Ok(db::BlockStatus::Success)
        }
        Err(e) => {
            db.set_block_status(block_number, db::BlockStatus::Error(e.clone()))?;
            // Always save of them for now, even when already cached.
            // TODO: avoid persisting when read from cache.
            db.set_block_traces(block_number, &traces_clone)?;
            debug!("Saved block traces for block {block_number}");
            Ok(db::BlockStatus::Error(e))
        }
    }
}

#[allow(clippy::too_many_arguments, unused_variables)]
fn run_block_with_retries(
    block_number: u64,
    db: &Database,
    endpoint: &str,
    witness_output_dir: Option<String>,
    persist_all: bool,
    chain_id: u64,
    single_tx: Option<u64>,
    gpu_shared_state: &mut Option<&mut GpuSharedState>,
    only_forward: bool,
    backup_endpoint: Option<&String>,
) -> Result<BlockStatus> {
    const MAX_RETRIES: usize = 3;

    for attempt in 1..=MAX_RETRIES {
        let endpoint = if attempt == 1 {
            endpoint
        } else if let Some(backup) = backup_endpoint {
            warn!("Switching to backup endpoint for block {block_number} on attempt {attempt}");
            backup.as_str()
        } else {
            endpoint
        };
        match run_block(
            block_number,
            db,
            endpoint,
            witness_output_dir.clone(), // avoid moving on first attempt
            persist_all,
            chain_id,
            single_tx,
            gpu_shared_state,
            only_forward,
        ) {
            core::result::Result::Ok(BlockStatus::Success) => return Ok(BlockStatus::Success),
            e if attempt < MAX_RETRIES => {
                warn!(
                    "Block {block_number} failed on attempt {attempt}/{MAX_RETRIES} with {e:?}, retrying..."
                );
            }
            e => {
                warn!("Block {block_number} failed after {MAX_RETRIES} attempts with {e:?}");
                return e;
            }
        }
    }

    unreachable!()
}

///
/// Run blocks from [start_block] to [end_block].
///
#[allow(clippy::too_many_arguments)]
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
    if let Some(webhook) = webhook.clone() {
        install_panic_hook(webhook);
    }
    let db = Database::init(db_path)?;
    assert!(start_block <= end_block);
    fetch_block_hashes(start_block, &db, &endpoint)?;
    let chain_id = rpc::get_chain_id(&endpoint)?;
    let mut failures = 0;

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
    let gpu_state = &mut None;

    for n in start_block..=end_block {
        let status = db.get_block_status(n)?;
        let already_succeeded = status.is_some_and(|s| matches!(s, BlockStatus::Success));
        if skip_successful && already_succeeded {
            debug!("Skipping block {n}, already succeeded");
            continue;
        }
        if let BlockStatus::Error(e) = run_block_with_retries(
            n,
            &db,
            &endpoint,
            witness_output_dir.clone(),
            persist_all,
            chain_id,
            single_tx,
            gpu_state,
            only_forward,
            backup_endpoint.as_ref(),
        )? {
            failures += 1;
            if let Some(webhook) = webhook.as_ref() {
                let msg = format!(":rotating_light: eth_runner: Block {n} on chain with id {chain_id} failed with: {e:?}");
                send_slack(webhook, &msg)?
            }
            if failures == MAX_FAILURES {
                error!("Reached max number of failures");
                panic!()
            }
        }
    }
    if let Some(webhook) = webhook.as_ref() {
        let msg = format!(":white_check_mark: eth_runner: finished running from block {start_block} to {end_block} on chain with id {chain_id} successfully!");
        send_slack(webhook, &msg)?
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
