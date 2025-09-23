use alloy::primitives::*;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnvSection {
    pub current_coinbase: Address,
    pub current_difficulty: Option<U256>,
    pub current_random: Option<U256>,
    pub current_base_fee: Option<U256>,
    pub current_gas_limit: U256,
    pub current_number: U256,
    pub current_timestamp: U256,
    pub previous_hash: Option<B256>,
}
