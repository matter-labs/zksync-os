use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use alloy::{hex, primitives::B256};
use anyhow::{Context, Result};
use log::debug;
use rig::zk_ee::utils::Bytes32;
use ruint::aliases::{B160, U256};

use crate::rpc_client::{Block, BlockMetadataResult, RpcClient, TransactionReceipt};

#[derive(Debug, Clone)]
pub struct LoadedBlockParams {
    pub block: Block,
    pub block_metadata: BlockMetadataResult,
    pub chain_id: u64,
    pub receipts: Vec<TransactionReceipt>,
    pub historical_block_hashes: [U256; 256],
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DiskStorageEntry {
    address: String,
    slot: String,
    value: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DiskPreimageEntry {
    hash: String,
    preimage: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DiskBlockParams {
    block_hash: String,
    block: Block,
    block_metadata: BlockMetadataResult,
    chain_id: u64,
    #[serde(default)]
    receipts: Vec<TransactionReceipt>,
    #[serde(default)]
    historical_block_hashes: Vec<String>,
}

pub fn block_params_cache_path(block_hash: B256) -> PathBuf {
    cache_dir().join(format!("block_params_{}.json", block_hash_key(block_hash)))
}

pub fn oracle_cache_paths(block_hash: B256) -> (PathBuf, PathBuf) {
    let block_hash_hex = block_hash_key(block_hash);
    let cache_dir = cache_dir();
    let storage = cache_dir.join(format!("storage_cache_{block_hash_hex}.json"));
    let preimages = cache_dir.join(format!("preimages_cache_{block_hash_hex}.json"));
    (storage, preimages)
}

pub fn load_or_fetch_block_params(
    rpc_client: &RpcClient,
    block_hash: B256,
    cache_path: &Path,
) -> Result<LoadedBlockParams> {
    match load_block_params_cache(cache_path, block_hash) {
        Ok(Some(cached)) => {
            let cached_block_tx_count = cached.block.tx_count();
            if cached.receipts.len() != cached_block_tx_count {
                eprintln!(
                    "Block params cache is stale (receipt count mismatch: receipts={}, txs={}), refetching",
                    cached.receipts.len(),
                    cached_block_tx_count
                );
            } else if cached.historical_block_hashes.len() != 256 {
                eprintln!(
                    "Block params cache is stale (historical block hash count mismatch: expected 256, got {}), refetching",
                    cached.historical_block_hashes.len()
                );
            } else {
                match decode_historical_block_hashes(&cached.historical_block_hashes) {
                    Ok(historical_block_hashes) => {
                        println!(
                            "Loaded block params from disk cache: {:?} (block_number={}, chain_id={}, receipts={}, historical_block_hashes={})",
                            cache_path,
                            cached.block.result.header.number,
                            cached.chain_id,
                            cached.receipts.len(),
                            cached.historical_block_hashes.len()
                        );
                        return Ok(LoadedBlockParams {
                            block: cached.block,
                            block_metadata: cached.block_metadata,
                            chain_id: cached.chain_id,
                            receipts: cached.receipts,
                            historical_block_hashes,
                        });
                    }
                    Err(err) => {
                        eprintln!(
                            "Block params cache is stale (invalid historical block hashes), refetching: {err}"
                        );
                    }
                }
            }
        }
        Ok(None) => {
            println!("Block params cache miss, fetching from RPC...");
        }
        Err(err) => {
            eprintln!("Failed to load block params cache, refetching from RPC: {err}");
        }
    }

    println!("Fetching block data...");
    let block = rpc_client.get_block_by_hash(block_hash)?;
    let block_number = block.result.header.number;
    println!("Fetched block number: {}", block_number);

    println!("Fetching block metadata...");
    let block_metadata = rpc_client.get_block_metadata(block_number)?;
    let chain_id = rpc_client.get_chain_id()?;
    let receipts = rpc_client.get_block_receipts(block_number)?.result;
    let historical_block_hashes = fetch_historical_block_hashes(rpc_client, block_number)?;

    if let Err(err) = save_block_params_cache(
        cache_path,
        block_hash,
        &block,
        &block_metadata,
        chain_id,
        &receipts,
        &historical_block_hashes,
    ) {
        eprintln!("Failed to save block params cache: {err}");
    } else {
        println!(
            "Saved block params cache to disk: {:?} (block_number={}, chain_id={}, receipts={}, historical_block_hashes={})",
            cache_path,
            block_number,
            chain_id,
            receipts.len(),
            historical_block_hashes.len()
        );
    }

    Ok(LoadedBlockParams {
        block,
        block_metadata,
        chain_id,
        receipts,
        historical_block_hashes,
    })
}

pub type StorageCache = HashMap<(B160, Bytes32), Bytes32>;
pub type PreimagesCache = HashMap<Bytes32, Vec<u8>>;

pub fn load_oracle_caches(
    storage_path: &Path,
    preimages_path: &Path,
) -> Result<(StorageCache, PreimagesCache)> {
    let mut storage_cache = HashMap::new();
    let mut preimages_cache = HashMap::new();

    if storage_path.exists() {
        let contents = std::fs::read(storage_path)
            .with_context(|| format!("failed to read storage cache file {:?}", storage_path))?;
        let entries: Vec<DiskStorageEntry> = serde_json::from_slice(&contents)
            .with_context(|| format!("failed to parse storage cache file {:?}", storage_path))?;
        for entry in entries {
            let address = B160::from_be_bytes(decode_fixed_hex::<20>(&entry.address)?);
            let slot = Bytes32::from(decode_fixed_hex::<32>(&entry.slot)?);
            let value = Bytes32::from(decode_fixed_hex::<32>(&entry.value)?);
            storage_cache.insert((address, slot), value);
        }
    }

    if preimages_path.exists() {
        let contents = std::fs::read(preimages_path)
            .with_context(|| format!("failed to read preimages cache file {:?}", preimages_path))?;
        let entries: Vec<DiskPreimageEntry> =
            serde_json::from_slice(&contents).with_context(|| {
                format!("failed to parse preimages cache file {:?}", preimages_path)
            })?;
        for entry in entries {
            let hash = Bytes32::from(decode_fixed_hex::<32>(&entry.hash)?);
            let preimage = decode_bytes_hex(&entry.preimage)?;
            preimages_cache.insert(hash, preimage);
        }
    }

    Ok((storage_cache, preimages_cache))
}

pub fn save_oracle_caches(
    storage_path: &Path,
    preimages_path: &Path,
    storage: &StorageCache,
    preimages: &PreimagesCache,
) -> Result<()> {
    let cache_dir = storage_path
        .parent()
        .context("storage cache path must have a parent directory")?;
    std::fs::create_dir_all(cache_dir)
        .with_context(|| format!("failed to create cache directory {:?}", cache_dir))?;

    let mut storage_entries: Vec<_> = storage
        .iter()
        .map(|((address, slot), value)| DiskStorageEntry {
            address: hex::encode_prefixed(address.to_be_bytes::<20>()),
            slot: hex::encode_prefixed(slot.as_u8_array_ref()),
            value: hex::encode_prefixed(value.as_u8_array_ref()),
        })
        .collect();
    storage_entries.sort_by(|a, b| {
        a.address
            .cmp(&b.address)
            .then_with(|| a.slot.cmp(&b.slot))
            .then_with(|| a.value.cmp(&b.value))
    });

    let mut preimage_entries: Vec<_> = preimages
        .iter()
        .map(|(hash, preimage)| DiskPreimageEntry {
            hash: hex::encode_prefixed(hash.as_u8_array_ref()),
            preimage: hex::encode_prefixed(preimage),
        })
        .collect();
    preimage_entries.sort_by(|a, b| a.hash.cmp(&b.hash));

    std::fs::write(storage_path, serde_json::to_vec_pretty(&storage_entries)?)
        .with_context(|| format!("failed to write storage cache file {:?}", storage_path))?;
    std::fs::write(
        preimages_path,
        serde_json::to_vec_pretty(&preimage_entries)?,
    )
    .with_context(|| format!("failed to write preimages cache file {:?}", preimages_path))?;

    Ok(())
}

fn cache_dir() -> PathBuf {
    PathBuf::from(".cache").join("block_reexecutor")
}

fn block_hash_key(block_hash: B256) -> String {
    format!("{block_hash:#x}")
        .trim_start_matches("0x")
        .to_owned()
}

fn load_block_params_cache(
    cache_path: &Path,
    expected_block_hash: B256,
) -> Result<Option<DiskBlockParams>> {
    if !cache_path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read(cache_path)
        .with_context(|| format!("failed to read block params cache file {:?}", cache_path))?;
    let cached: DiskBlockParams = serde_json::from_slice(&contents)
        .with_context(|| format!("failed to parse block params cache file {:?}", cache_path))?;

    let expected_hash = block_hash_key(expected_block_hash);
    let cached_hash = cached.block_hash.trim_start_matches("0x");
    if cached_hash != expected_hash {
        return Err(anyhow::anyhow!(
            "block hash mismatch in cache file {:?}: expected 0x{}, got {}",
            cache_path,
            expected_hash,
            cached.block_hash
        ));
    }

    Ok(Some(cached))
}

fn save_block_params_cache(
    cache_path: &Path,
    block_hash: B256,
    block: &Block,
    block_metadata: &BlockMetadataResult,
    chain_id: u64,
    receipts: &[TransactionReceipt],
    historical_block_hashes: &[U256; 256],
) -> Result<()> {
    let cache_dir = cache_path
        .parent()
        .context("block params cache path must have a parent directory")?;
    std::fs::create_dir_all(cache_dir)
        .with_context(|| format!("failed to create cache directory {:?}", cache_dir))?;

    let payload = DiskBlockParams {
        block_hash: format!("{block_hash:#x}"),
        block: block.clone(),
        block_metadata: block_metadata.clone(),
        chain_id,
        receipts: receipts.to_vec(),
        historical_block_hashes: encode_historical_block_hashes(historical_block_hashes),
    };

    std::fs::write(cache_path, serde_json::to_vec_pretty(&payload)?)
        .with_context(|| format!("failed to write block params cache file {:?}", cache_path))?;

    Ok(())
}

fn decode_fixed_hex<const N: usize>(value: &str) -> Result<[u8; N]> {
    let stripped = value.strip_prefix("0x").unwrap_or(value);
    let raw =
        hex::decode(stripped).with_context(|| format!("failed to decode hex value `{value}`"))?;
    let raw_len = raw.len();
    raw.try_into()
        .map_err(|_| anyhow::anyhow!("expected {N} bytes, got {}", raw_len))
}

fn decode_bytes_hex(value: &str) -> Result<Vec<u8>> {
    let stripped = value.strip_prefix("0x").unwrap_or(value);
    let raw =
        hex::decode(stripped).with_context(|| format!("failed to decode hex value `{value}`"))?;
    Ok(raw)
}

fn fetch_historical_block_hashes(rpc_client: &RpcClient, block_number: u64) -> Result<[U256; 256]> {
    let mut block_hashes = [U256::ZERO; 256];
    let mut loaded = 0usize;

    for depth in 1u64..=256 {
        debug!("Fetching historical block hash: depth {depth}");
        let Some(target_block_number) = block_number.checked_sub(depth) else {
            break;
        };

        let idx = 256 - depth as usize;
        match rpc_client.get_block_hash_by_number(target_block_number)? {
            Some(hash) => {
                block_hashes[idx] = U256::from_be_bytes(hash.0);
                loaded += 1;
            }
            None => {
                println!(
                    "RPC returned null for historical block #{target_block_number}; leaving remaining block hashes as zeroes"
                );
                break;
            }
        }
    }

    println!("Fetched {} historical block hashes from RPC", loaded);
    Ok(block_hashes)
}

fn encode_historical_block_hashes(hashes: &[U256; 256]) -> Vec<String> {
    hashes
        .iter()
        .map(|hash| hex::encode_prefixed(hash.to_be_bytes::<32>()))
        .collect()
}

fn decode_historical_block_hashes(raw_hashes: &[String]) -> Result<[U256; 256]> {
    if raw_hashes.len() != 256 {
        return Err(anyhow::anyhow!(
            "expected 256 historical block hashes, got {}",
            raw_hashes.len()
        ));
    }

    let mut hashes = [U256::ZERO; 256];
    for (idx, raw_hash) in raw_hashes.iter().enumerate() {
        hashes[idx] = U256::from_be_bytes(decode_fixed_hex::<32>(raw_hash)?);
    }
    Ok(hashes)
}
