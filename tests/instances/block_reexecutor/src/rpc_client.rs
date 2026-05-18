use alloy::{
    primitives::{Address, Bloom, Bytes, B256, U256},
    rpc::types::Transaction,
};
use anyhow::{anyhow, Result};
use log::{debug, warn};
use rig::{utils::encode_alloy_rpc_tx, zksync_os_interface::traits::EncodedTx};
use ruint::aliases::B160;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io::Read, str::FromStr, thread, time::Duration};
use ureq::json;

/// Simple RPC client with the basic methods we need
pub struct RpcClient {
    endpoint: String,
}

impl RpcClient {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    /// Converts u64 to hex string with "0x" prefix
    fn to_hex(n: u64) -> String {
        format!("0x{n:x}")
    }

    /// Send JSON-RPC request
    fn send(&self, body: Value) -> Result<String> {
        const MAX_502_RETRIES: usize = 3;
        const RETRY_DELAY: Duration = Duration::from_secs(3);

        for attempt in 0..=MAX_502_RETRIES {
            match ureq::post(&self.endpoint)
                .set("Content-Type", "application/json")
                .send_json(body.clone())
            {
                Ok(response) => {
                    let mut out = String::new();
                    response.into_reader().read_to_string(&mut out)?;
                    return Ok(out);
                }
                Err(ureq::Error::Status(502, _)) if attempt < MAX_502_RETRIES => {
                    warn!(
                        "RPC returned 502 (attempt {}/{}), retrying in {}s",
                        attempt + 1,
                        MAX_502_RETRIES + 1,
                        RETRY_DELAY.as_secs()
                    );
                    thread::sleep(RETRY_DELAY);
                }
                Err(err) => return Err(err.into()),
            }
        }

        unreachable!("retry loop must either return success or an error")
    }

    /// Fetches the full block data with transactions.
    pub fn get_block_by_hash(&self, block_hash: B256) -> Result<Block> {
        debug!("RPC: get_block_by_hash({:?})", block_hash);
        let body = json!({
            "method": "eth_getBlockByHash",
            "params": [format!("0x{:x}", block_hash), true],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let block = serde_json::from_str(&res)?;
        Ok(block)
    }

    /// Fetches the full block data by number with transactions.
    pub fn get_block_by_number(&self, block_number: u64) -> Result<Block> {
        debug!("RPC: get_block_by_number({block_number})");
        let body = json!({
            "method": "eth_getBlockByNumber",
            "params": [Self::to_hex(block_number), true],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let block = serde_json::from_str(&res)?;
        Ok(block)
    }

    /// Fetch block hash by number.
    pub fn get_block_hash_by_number(&self, block_number: u64) -> Result<Option<B256>> {
        debug!("RPC: get_block_hash_by_number({block_number})");
        let body = json!({
            "method": "eth_getBlockByNumber",
            "params": [Self::to_hex(block_number), false],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let response: Value = serde_json::from_str(&res)?;
        let Some(block) = response.get("result") else {
            return Err(anyhow!("No result field in eth_getBlockByNumber response"));
        };
        if block.is_null() {
            return Ok(None);
        }
        let hash_hex = block["hash"]
            .as_str()
            .ok_or_else(|| anyhow!("No block hash in eth_getBlockByNumber response"))?;
        let hash = B256::from_str(hash_hex)?;
        Ok(Some(hash))
    }

    /// Fetches the ZKsync OS specific block metadata.
    pub fn get_block_metadata(&self, block_number: u64) -> Result<BlockMetadataResult> {
        debug!("RPC: zks_getBlockMetadataByNumber({block_number})");
        let body = json!({
            "method": "zks_getBlockMetadataByNumber",
            "params": [block_number],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        println!("Raw block metadata response: {}", res);
        let block = serde_json::from_str(&res)?;
        Ok(block)
    }

    pub fn get_chain_id(&self) -> Result<u64> {
        debug!("RPC: get_chain_id()");
        let body = json!({
            "method": "eth_chainId",
            "params": [],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let response: Value = serde_json::from_str(&res)?;
        let chain_id_hex = response["result"]
            .as_str()
            .ok_or_else(|| anyhow!("No chain ID found in response"))?;
        let chain_id = u64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16)?;
        Ok(chain_id)
    }

    /// Fetch account balance
    pub fn get_balance(&self, address: Address, block_number: u64) -> Result<U256> {
        debug!("RPC: get_balance({address}, {block_number})");
        let body = json!({
            "method": "eth_getBalance",
            "params": [format!("0x{:x}", address), Self::to_hex(block_number)],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let response: Value = serde_json::from_str(&res)?;
        let balance_hex = response["result"]
            .as_str()
            .ok_or_else(|| anyhow!("No balance found in response"))?;
        let balance = U256::from_str(balance_hex)?;
        Ok(balance)
    }

    /// Fetch contract code
    pub fn get_code(&self, address: Address, block_number: u64) -> Result<Bytes> {
        debug!("RPC: get_code({address}, {block_number})");
        let body = json!({
            "method": "eth_getCode",
            "params": [format!("0x{:x}", address), Self::to_hex(block_number)],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let response: Value = serde_json::from_str(&res)?;
        let code_hex = response["result"]
            .as_str()
            .ok_or_else(|| anyhow!("No code found in response"))?;
        let code = Bytes::from_str(code_hex)?;
        Ok(code)
    }

    /// Fetch transaction count (nonce)
    pub fn get_transaction_count(&self, address: Address, block_number: u64) -> Result<u64> {
        debug!("RPC: get_transaction_count({address}, {block_number})");
        let body = json!({
            "method": "eth_getTransactionCount",
            "params": [format!("0x{:x}", address), Self::to_hex(block_number)],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let response: Value = serde_json::from_str(&res)?;
        let nonce_hex = response["result"]
            .as_str()
            .ok_or_else(|| anyhow!("No transaction count found in response"))?;
        let nonce = u64::from_str_radix(nonce_hex.trim_start_matches("0x"), 16)?;
        Ok(nonce)
    }

    /// Fetch storage value
    pub fn get_storage_at(&self, address: Address, slot: U256, block_number: u64) -> Result<U256> {
        debug!("RPC: get_storage_at({address}, {slot}, {block_number})");
        let body = json!({
            "method": "eth_getStorageAt",
            "params": [format!("0x{:x}", address), format!("0x{:x}", slot), Self::to_hex(block_number)],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let response: Value = serde_json::from_str(&res)?;
        let storage_hex = response["result"]
            .as_str()
            .ok_or_else(|| anyhow!("No storage value found in response"))?;
        let storage = U256::from_str(storage_hex)?;
        Ok(storage)
    }

    /// Fetch receipts for all block transactions.
    pub fn get_block_receipts(&self, block_number: u64) -> Result<BlockReceipts> {
        debug!("RPC: get_block_receipts({block_number})");
        let body = json!({
            "method": "eth_getBlockReceipts",
            "params": [Self::to_hex(block_number)],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let receipts = serde_json::from_str(&res)?;
        Ok(receipts)
    }

    /// Fetch transaction receipt by tx hash.
    pub fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<TransactionReceipt>> {
        debug!("RPC: get_transaction_receipt({tx_hash:#x})");
        let body = json!({
            "method": "eth_getTransactionReceipt",
            "params": [format!("0x{:x}", tx_hash)],
            "id": 1,
            "jsonrpc": "2.0"
        });
        let res = self.send(body)?;
        let receipt: MaybeTransactionReceipt = serde_json::from_str(&res)?;
        Ok(receipt.result)
    }
}

use alloy::eips::Typed2718;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Block {
    pub result: alloy::rpc::types::Block<alloy::rpc::types::Transaction, alloy::rpc::types::Header>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BlockMetadataResult {
    pub result: BlockMetadata,
}

/// ZKsync-specific block metadata struct.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockMetadata {
    pub pubdata_price_per_byte: U256,
    pub native_price: U256,
    pub execution_version: u32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionReceipt {
    pub transaction_hash: B256,
    pub transaction_index: U256,
    pub block_hash: B256,
    pub block_number: U256,
    pub from: Address,
    pub to: Option<Address>,
    pub cumulative_gas_used: U256,
    pub gas_used: U256,
    pub contract_address: Option<Address>,
    pub logs: Vec<ReceiptLog>,
    pub logs_bloom: Bloom,
    pub status: Option<U256>,
    #[serde(rename = "type")]
    pub tx_type: Option<U256>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptLog {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Bytes,
    pub block_number: U256,
    pub transaction_hash: B256,
    pub transaction_index: U256,
    pub block_hash: B256,
    pub log_index: U256,
    pub removed: Option<bool>,
}

impl ReceiptLog {
    pub fn is_equal_to_excluding_data(&self, log: &rig::zksync_os_interface::types::Log) -> bool {
        let address_check = || self.address == log.address;
        let topics_length_check = || self.topics.len() == log.topics().len();
        let topics_check = || {
            self.topics
                .iter()
                .zip(log.topics().iter())
                .all(|(l, r)| l == r)
        };
        address_check() && topics_length_check() && topics_check()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BlockReceipts {
    pub result: Vec<TransactionReceipt>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MaybeTransactionReceipt {
    pub result: Option<TransactionReceipt>,
}

impl Block {
    pub fn get_block_context(&self) -> rig::BlockContext {
        let base_fee = U256::from(self.result.header.base_fee_per_gas.unwrap_or(1000));
        rig::BlockContext {
            timestamp: self.result.header.timestamp,
            eip1559_basefee: base_fee,
            pubdata_price: U256::ZERO,
            native_price: base_fee / U256::from(100),
            coinbase: B160::from_be_bytes(self.result.header.beneficiary.0 .0),
            gas_limit: self.result.header.gas_limit,
            pubdata_limit: u64::MAX,
            mix_hash: U256::from_be_bytes(self.result.header.mix_hash.0),
        }
    }

    pub fn get_transactions(self) -> Vec<EncodedTx> {
        self.result
            .transactions
            .into_transactions()
            .filter_map(|tx| {
                let transaction_type = tx.ty();
                let supported_tx_type = transaction_type <= 2;
                if supported_tx_type {
                    Some(encode_alloy_rpc_tx(tx))
                } else {
                    warn!("Skipping unsupported transaction of type {transaction_type:?}");
                    None
                }
            })
            .collect()
    }

    pub fn get_transactions_raw(self) -> Vec<Transaction> {
        self.result
            .transactions
            .into_transactions()
            .filter_map(|tx| {
                let transaction_type = tx.ty();
                let supported_tx_type = transaction_type <= 2;
                if supported_tx_type {
                    Some(tx)
                } else {
                    warn!("Skipping unsupported transaction of type {transaction_type:?}");
                    None
                }
            })
            .collect()
    }
}
