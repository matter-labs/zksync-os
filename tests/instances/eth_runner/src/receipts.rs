use alloy::primitives::{Address, Bloom, B256, U256};
use serde::{Deserialize, Serialize};

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
    pub logs: Vec<Log>,
    pub logs_bloom: Bloom,
    pub status: Option<U256>,
    #[serde(rename = "type")]
    pub tx_type: Option<U256>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: alloy::primitives::Bytes,
    pub block_number: U256,
    pub transaction_hash: B256,
    pub transaction_index: U256,
    pub block_hash: B256,
    pub log_index: U256,
    pub removed: Option<bool>,
}

impl Log {
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
