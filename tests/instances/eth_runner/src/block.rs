use std::collections::HashSet;

use crate::calltrace::CallTrace;
use alloy::eips::Typed2718;
use rig::forward_system::run::convert_alloy::FromAlloy;
use rig::zksync_os_tests_common::zksync_tx::encoding::encode_alloy_rpc_tx;
use rig::{log::warn, zksync_os_interface::traits::EncodedTx};
use ruint::aliases::{B160, U256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Block {
    pub result: alloy::rpc::types::Block<alloy::rpc::types::Transaction, alloy::rpc::types::Header>,
}

impl Block {
    pub fn get_block_context(&self) -> rig::BlockContext {
        let base_fee = U256::from(self.result.header.base_fee_per_gas.unwrap_or(1000));
        let blob_fee = self
            .result
            .header
            .blob_fee()
            .map(U256::from)
            .unwrap_or(U256::MAX);
        rig::BlockContext {
            timestamp: self.result.header.timestamp,
            eip1559_basefee: base_fee,
            pubdata_price: U256::ZERO,
            native_price: (base_fee / U256::from(100)).max(U256::ONE),
            coinbase: B160::from_alloy(self.result.header.beneficiary),
            gas_limit: self.result.header.gas_limit,
            pubdata_limit: u64::MAX,
            mix_hash: U256::from_be_bytes(self.result.header.mix_hash.0),
            blob_fee,
        }
    }

    /// Returns (transactions, skipped, has_call_to_unsupported_precompile)
    pub fn get_transactions(
        self,
        calltrace: &CallTrace,
        single_tx: Option<u64>,
    ) -> (Vec<EncodedTx>, HashSet<usize>, bool) {
        let mut skipped: HashSet<usize> = HashSet::new();
        let mut has_call_to_unsupported_precompile = false;
        (
            self.result
                .transactions
                .into_transactions()
                .enumerate()
                .zip(calltrace.result.iter())
                .filter_map(|((i, tx), calltrace)| {
                    // Skip unsupported txs or tx that call into unsupported precompiles

                    let transaction_type = tx.ty();
                    let supported_tx_type = transaction_type <= 3;
                    let single_tx_cond = single_tx.is_none_or(|idx| idx as usize == i);
                    let unsupported_precompile =
                        calltrace.result.has_call_to_unsupported_precompile();
                    has_call_to_unsupported_precompile |= unsupported_precompile;
                    if single_tx_cond && supported_tx_type && !unsupported_precompile {
                        Some(encode_alloy_rpc_tx(tx))
                    } else {
                        warn!("Skipping unsupported transaction of type {transaction_type:?}");
                        skipped.insert(i);
                        None
                    }
                })
                .collect(),
            skipped,
            has_call_to_unsupported_precompile,
        )
    }
}
