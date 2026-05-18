use alloy::{
    consensus::{transaction::Recovered, TxEnvelope},
    eips::eip2718::Decodable2718,
    hex,
    primitives::{Address, B256},
    rpc::types::{trace::geth::CallFrame, Transaction as RpcTransaction},
};
use anyhow::{Context, Result};
use clap::Parser;
use std::{collections::HashMap, path::PathBuf};

mod cache;
mod rpc_client;
mod rpc_oracle;
mod state_view;
mod tx_check;

use cache::{
    block_params_cache_path, load_or_fetch_block_params, load_oracle_caches, oracle_cache_paths,
    save_oracle_caches,
};
use rig::zksync_os_interface::traits::EncodedTx;
use rig::{
    chain::RunConfig, forward_system::system::tracers::call_tracer::CallTracer,
    zk_ee::common_structs::derive_flat_storage_key,
};
use rpc_client::RpcClient;
use state_view::{generate_block_context_interface, ChainStateView};
use tx_check::{
    check_selected_tx_outputs_against_receipts, check_tx_outputs_against_receipts,
    filter_supported_receipts,
};
use zksync_os_revm_runner::revm_runner;

#[derive(Parser, Debug)]
#[command(author, version, about = "Re-execute blocks using external RPC")]
struct Args {
    /// RPC endpoint URL
    #[arg(long, default_value = "http://localhost:8545")]
    endpoint: String,

    /// Block hash to re-execute (required in default mode)
    #[arg(long)]
    block_hash: Option<B256>,

    /// Block number (required when --transactions-file is set)
    #[arg(long)]
    block_number: Option<u64>,

    /// Enable verbose logging
    #[arg(long, short)]
    verbose: bool,

    /// Optional JSON file with predefined transactions encoded as `EncodedTx::Rlp`.
    /// When set, these txs are executed against the state at `--block-number`.
    #[arg(long)]
    transactions_file: Option<PathBuf>,
}

enum ReceiptCheckMode {
    FullBlock,
    PredefinedByHash(Vec<Option<B256>>),
    None,
}

fn main() -> Result<()> {
    let args = Args::parse();

    rig::init_logger();

    println!("Starting block re-execution");
    println!("Endpoint: {}", args.endpoint);
    println!("Block hash: {:?}", args.block_hash);
    println!("Block number: {:?}", args.block_number);

    let rpc_client = RpcClient::new(args.endpoint.clone());
    let block_hash = if args.transactions_file.is_some() {
        let block_number = args
            .block_number
            .context("--block-number is required when --transactions-file is set")?;
        let block = rpc_client.get_block_by_number(block_number)?;
        let resolved_hash = block.result.header.hash;
        println!(
            "Resolved block number {} to block hash {:?}",
            block_number, resolved_hash
        );
        resolved_hash
    } else {
        args.block_hash
            .context("--block-hash is required unless --transactions-file is set")?
    };

    let block_params_path = block_params_cache_path(block_hash);
    let loaded = load_or_fetch_block_params(&rpc_client, block_hash, &block_params_path)?;
    let block = loaded.block;
    let block_metadata = loaded.block_metadata;
    let chain_id = loaded.chain_id;
    let cached_or_fetched_receipts = loaded.receipts;
    let historical_block_hashes = loaded.historical_block_hashes;

    let block_number = block.result.header.number;
    let miner = block.result.header.beneficiary;
    let mut block_context = block.get_block_context();

    println!("Native price: {}", block_metadata.result.native_price);
    println!(
        "Pubdata price: {}",
        block_metadata.result.pubdata_price_per_byte
    );

    block_context.native_price = block_metadata.result.native_price;
    block_context.pubdata_price = block_metadata.result.pubdata_price_per_byte;

    let (transactions, raw_transactions, receipt_check_mode) =
        if let Some(path) = args.transactions_file.as_ref() {
            let (encoded, raw, tx_hashes) = load_predefined_rlp_transactions(path)?;
            let check_mode = if tx_hashes.iter().any(|hash| hash.is_some()) {
                ReceiptCheckMode::PredefinedByHash(tx_hashes)
            } else {
                ReceiptCheckMode::None
            };
            (encoded, raw, check_mode)
        } else {
            (
                block.clone().get_transactions(),
                block.clone().get_transactions_raw(),
                ReceiptCheckMode::FullBlock,
            )
        };

    println!(
        "Block {} has {} transactions",
        block_number,
        transactions.len()
    );
    println!("Block hash: {:?}", block.result.header.hash);
    println!("Block miner: {:?}", miner);
    println!(
        "Block gas used: {} / {}",
        block.result.header.gas_used, block.result.header.gas_limit
    );

    if transactions.is_empty() {
        println!("No transactions to execute, skipping block");
        return Ok(());
    }

    println!(
        "Block context: timestamp={}, gas_limit={}, coinbase={:?}",
        block_context.timestamp, block_context.gas_limit, block_context.coinbase
    );

    let oracle_factory = rpc_oracle::RpcValueOracleFactory::new(args.endpoint, block_number);
    let (storage_cache_path, preimages_cache_path) = oracle_cache_paths(block_hash);
    let (cached_storage, cached_preimages) =
        match load_oracle_caches(&storage_cache_path, &preimages_cache_path) {
            Ok(caches) => caches,
            Err(err) => {
                eprintln!("Failed to load oracle cache from disk: {err}");
                (HashMap::new(), HashMap::new())
            }
        };
    if !cached_storage.is_empty() || !cached_preimages.is_empty() {
        println!(
            "Loaded oracle cache from disk: storage_slots={}, preimages={}",
            cached_storage.len(),
            cached_preimages.len()
        );
    }

    oracle_factory
        .cache
        .lock()
        .expect("failed to lock oracle cache")
        .extend(cached_storage);
    oracle_factory
        .preimages
        .lock()
        .expect("failed to lock oracle preimages")
        .extend(cached_preimages);

    let mut chain = rig::Chain::empty(Some(chain_id));
    if block_number > 0 {
        chain.set_last_block_number(block_number - 1);
    }
    chain.set_block_hashes(historical_block_hashes);
    chain.set_timestamp(block_context.timestamp);

    println!(
        "Running block with {} transactions using RPC oracle...",
        transactions.len()
    );

    let run_config = Some(RunConfig {
        profiler_config: None,
        witness_output_file: None,
        app: None,
        only_forward: true,
        check_storage_diff_hashes: false,
        not_update_state_after_block_execution: true,
    });

    let mut tracer = CallTracer::default();

    let block_output = chain.run_block_with_oracle_factory_and_tracer(
        transactions,
        Some(block_context.clone()),
        None,
        run_config,
        &oracle_factory,
        &mut tracer,
    );

    match receipt_check_mode {
        ReceiptCheckMode::FullBlock => {
            let receipts = filter_supported_receipts(&block, cached_or_fetched_receipts)?;
            check_tx_outputs_against_receipts(&block_output, &receipts)?;
            println!(
                "Transaction output check passed against {} cached RPC receipts",
                receipts.len()
            );
        }
        ReceiptCheckMode::PredefinedByHash(tx_hashes) => {
            let mut expected_receipts = Vec::new();
            let mut should_skip_check = false;
            for (tx_idx, maybe_hash) in tx_hashes.into_iter().enumerate() {
                if let Some(hash) = maybe_hash {
                    match rpc_client.get_transaction_receipt(hash)? {
                        Some(receipt) => expected_receipts.push((tx_idx, receipt)),
                        None => {
                            println!(
                                "RPC returned null for receipt hash {hash:#x}; skipping receipt checks for predefined transaction mode"
                            );
                            should_skip_check = true;
                            break;
                        }
                    }
                }
            }

            if should_skip_check {
                println!("Receipt checks skipped because RPC returned null receipt");
            } else if expected_receipts.is_empty() {
                println!("Skipping RPC receipt checks: predefined tx input has no hashes");
            } else {
                check_selected_tx_outputs_against_receipts(&block_output, &expected_receipts)?;
                println!(
                    "Transaction output check passed against {} hash-matched RPC receipts",
                    expected_receipts.len()
                );
            }
        }
        ReceiptCheckMode::None => {
            println!("Skipping RPC receipt checks for predefined transaction mode");
        }
    }

    let preimages = oracle_factory
        .preimages
        .lock()
        .expect("failed to lock oracle preimages")
        .clone();
    let storage = oracle_factory
        .cache
        .lock()
        .expect("failed to lock oracle cache")
        .clone();

    save_oracle_caches(
        &storage_cache_path,
        &preimages_cache_path,
        &storage,
        &preimages,
    )?;
    println!(
        "Saved oracle cache to disk: storage_slots={}, preimages={}",
        storage.len(),
        preimages.len()
    );

    for (hash, preimage) in &preimages {
        chain.preimage_source.inner.insert(*hash, preimage.clone());
    }

    for ((address, slot), value) in &storage {
        let flat_key = derive_flat_storage_key(&address, &slot);
        chain.state_tree.cold_storage.insert(flat_key, *value);
    }

    println!("Block execution completed successfully!");
    println!("Block output: gas_used = {}", block_output.header.gas_used);
    println!("Block output: transactions = {:?}", block_output.tx_results);

    println!("Block re-execution completed");

    let trace = tracer
        .transactions
        .into_iter()
        .map(CallFrame::from)
        .collect::<Vec<_>>();
    let trace = apply_root_gas_used_from_block_output(trace, &block_output);
    let trace = trace
        .into_iter()
        .map(normalize_call_frame_for_geth_output)
        .collect::<Vec<_>>();

    let tracer_output_path = format!("tracer_output_{}.json", block_number);
    std::fs::write(&tracer_output_path, serde_json::to_string_pretty(&trace)?)?;
    println!("Tracer output saved to {}", tracer_output_path);

    println!("Runnning ZKsync OS REVM");

    let block_context = generate_block_context_interface(&chain, &block_context);
    println!("Generated block context for REVM: {:?}", block_context);
    let state_view = ChainStateView { chain };

    let mut revm_runner = revm_runner::RevmRunner::new(state_view);

    let revm_trace =
        revm_runner.run_with_call_traces(raw_transactions, block_context, Some(block_output))?;
    let revm_trace_output_path = format!("revm_call_trace_{}.json", block_number);
    std::fs::write(
        &revm_trace_output_path,
        serde_json::to_string_pretty(&revm_trace)?,
    )?;
    println!("REVM call trace saved to {}", revm_trace_output_path);

    Ok(())
}

fn apply_root_gas_used_from_block_output(
    mut traces: Vec<CallFrame>,
    block_output: &rig::zksync_os_interface::types::BlockOutput,
) -> Vec<CallFrame> {
    if traces.len() != block_output.tx_results.len() {
        eprintln!(
            "trace/result length mismatch: traces={} tx_results={}",
            traces.len(),
            block_output.tx_results.len()
        );
    }

    for (frame, tx_result) in traces.iter_mut().zip(block_output.tx_results.iter()) {
        if let Ok(tx_output) = tx_result {
            frame.gas_used = alloy::primitives::U256::from(tx_output.gas_used);
        }
    }

    traces
}

fn normalize_call_frame_for_geth_output(mut frame: CallFrame) -> CallFrame {
    frame.calls = frame
        .calls
        .into_iter()
        .map(normalize_call_frame_for_geth_output)
        .collect();

    if matches!(frame.typ.as_str(), "STATICCALL") {
        frame.value = None;
    }

    if frame
        .output
        .as_ref()
        .is_some_and(|output| output.is_empty())
    {
        frame.output = None;
    }
    frame
}

fn load_predefined_rlp_transactions(
    path: &PathBuf,
) -> Result<(Vec<EncodedTx>, Vec<RpcTransaction>, Vec<Option<B256>>)> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read predefined txs file {:?}", path))?;
    let encoded_txs: Vec<PredefinedTxJson> = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse predefined txs file {:?}", path))?;

    if encoded_txs.is_empty() {
        anyhow::bail!("predefined tx list from {:?} is empty", path);
    }

    let mut rig_txs = Vec::with_capacity(encoded_txs.len());
    let mut revm_txs = Vec::with_capacity(encoded_txs.len());
    let mut tx_hashes = Vec::with_capacity(encoded_txs.len());
    for (idx, encoded_tx) in encoded_txs.into_iter().enumerate() {
        let (rlp, signer, tx_hash) = match encoded_tx {
            PredefinedTxJson::EncodedTx(EncodedTx::Rlp(rlp, signer)) => (rlp, signer, None),
            PredefinedTxJson::EncodedTx(_) => {
                anyhow::bail!(
                    "predefined tx #{idx} has non-RLP encoding; only EncodedTx::Rlp is supported"
                );
            }
            PredefinedTxJson::TaggedRlpHex {
                rlp: (tx_hex, signer),
                hash,
            } => (decode_tx_hex(&tx_hex, idx)?, signer, hash),
            PredefinedTxJson::TaggedRlpBytes {
                rlp: (rlp, signer),
                hash,
            } => (rlp, signer, hash),
            PredefinedTxJson::FlatHex { rlp, signer, hash } => {
                (decode_tx_hex(&rlp, idx)?, signer, hash)
            }
            PredefinedTxJson::FlatTx { tx, signer, hash } => {
                (decode_tx_hex(&tx, idx)?, signer, hash)
            }
        };

        let envelope = TxEnvelope::decode_2718_exact(&rlp)
            .with_context(|| format!("failed to decode RLP tx at index {idx}"))?;

        let rpc_tx = RpcTransaction {
            inner: Recovered::new_unchecked(envelope, signer),
            block_hash: None,
            block_number: None,
            transaction_index: Some(idx as u64),
            effective_gas_price: None,
        };

        rig_txs.push(EncodedTx::Rlp(rlp, signer));
        revm_txs.push(rpc_tx);
        tx_hashes.push(tx_hash);
    }

    let hash_count = tx_hashes.iter().filter(|hash| hash.is_some()).count();
    println!(
        "Loaded {} predefined RLP transactions from {:?} ({} with expected receipt hashes)",
        rig_txs.len(),
        path,
        hash_count
    );

    Ok((rig_txs, revm_txs, tx_hashes))
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum PredefinedTxJson {
    // Backward-compatible shape: {"Rlp":[[u8,u8,...], "0x..."]}
    EncodedTx(EncodedTx),
    // Hex-compatible external-tag shape: {"Rlp":["0x...", "0x..."]}
    TaggedRlpHex {
        #[serde(rename = "Rlp")]
        rlp: (String, Address),
        #[serde(default)]
        hash: Option<B256>,
    },
    // Backward-compatible external-tag shape with bytes: {"Rlp":[[u8,...], "0x..."]}
    TaggedRlpBytes {
        #[serde(rename = "Rlp")]
        rlp: (Vec<u8>, Address),
        #[serde(default)]
        hash: Option<B256>,
    },
    // Flat shape: {"rlp":"0x...", "signer":"0x..."}
    FlatHex {
        rlp: String,
        signer: Address,
        #[serde(default)]
        hash: Option<B256>,
    },
    // Flat alias: {"tx":"0x...", "signer":"0x..."}
    FlatTx {
        tx: String,
        signer: Address,
        #[serde(default)]
        hash: Option<B256>,
    },
}

fn decode_tx_hex(tx_hex: &str, idx: usize) -> Result<Vec<u8>> {
    let tx_hex = tx_hex.strip_prefix("0x").unwrap_or(tx_hex);
    hex::decode(tx_hex).with_context(|| format!("failed to decode tx hex at index {idx}"))
}
