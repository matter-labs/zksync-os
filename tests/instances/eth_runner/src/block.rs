use std::collections::HashSet;

use rig::log::warn;
use rig::utils::encode_alloy_rpc_tx;
use ruint::aliases::{B160, U256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Block {
    pub result: alloy::rpc::types::Block<alloy::rpc::types::Transaction, alloy::rpc::types::Header>,
}

impl Block {
    pub fn get_block_context(&self) -> rig::BlockContext {
        let base_fee = U256::from(self.result.header.base_fee_per_gas.unwrap_or(1000));
        rig::BlockContext {
            timestamp: self.result.header.timestamp,
            eip1559_basefee: base_fee,
            gas_per_pubdata: U256::ZERO,
            native_price: base_fee / U256::from(100),
            coinbase: B160::from_be_bytes(self.result.header.miner.0 .0),
            gas_limit: self.result.header.gas_limit,
            mix_hash: self
                .result
                .header
                .mix_hash
                .map(|b| U256::from_be_bytes(b.0))
                .unwrap_or(U256::ONE),
        }
    }

    pub fn get_transactions(self) -> (Vec<Vec<u8>>, HashSet<usize>) {
        let mut skipped: HashSet<usize> = HashSet::new();
        (
            self.result
                .transactions
                .into_transactions()
                .enumerate()
                .filter_map(|(i, tx)| {
                    // Skip unsupported txs
                    if tx.transaction_type.is_none_or(|t| t == 0u8)
                        || tx.transaction_type == Some(1u8)
                        || tx.transaction_type == Some(2u8)
                    {
                        Some(encode_alloy_rpc_tx(tx))
                    } else {
                        warn!(
                            "Skipping unsupported transaction of type {:?}",
                            tx.transaction_type
                        );
                        skipped.insert(i);
                        None
                    }
                })
                .collect(),
            skipped,
        )
    }
}
