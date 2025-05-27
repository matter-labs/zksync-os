use std::collections::HashSet;

use rig::utils::encode_alloy_rpc_tx;
use ruint::aliases::{B160, U256};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
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
                    if tx.access_list.as_ref().is_none_or(|l| l.is_empty()) {
                        Some(encode_alloy_rpc_tx(tx))
                    } else {
                        skipped.insert(i);
                        None
                    }
                })
                .collect(),
            skipped,
        )
    }
}
