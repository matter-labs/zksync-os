use std::collections::HashSet;

use crate::calltrace::CallTrace;
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

    pub fn get_transactions(self, calltrace: &CallTrace) -> (Vec<Vec<u8>>, HashSet<usize>) {
        let mut skipped: HashSet<usize> = HashSet::new();
        (
            self.result
                .transactions
                .into_transactions()
                .enumerate()
                .zip(calltrace.result.iter())
                .filter_map(|((i, tx), calltrace)| {
                    // Skip unsupported txs or tx that call into unsupported precompiles
                    let calls_unsupported_percompile =
                        || calltrace.result.has_call_to_unsupported_precompile();
                    let supported_tx_type = tx.transaction_type.is_none_or(|t| t == 0u8)
                        || tx.transaction_type == Some(1u8)
                        || tx.transaction_type == Some(2u8);
                    if supported_tx_type && !calls_unsupported_percompile() {
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
