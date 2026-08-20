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

// Blob base fee update fraction. Must match the STF's active blob schedule:
// base-Osaka 5007716, or BPO2 (`fusaka-bpo-2`) 11684671. Alloy's
// `Header::blob_fee()` uses a fixed fraction that does not track the BPO
// schedule, so we compute the blob base fee ourselves from `excessBlobGas`
// to keep the replay consistent with the built STF.
#[cfg(not(feature = "fusaka-bpo-2"))]
const BLOB_BASE_FEE_UPDATE_FRACTION: u128 = 5_007_716;
#[cfg(feature = "fusaka-bpo-2")]
const BLOB_BASE_FEE_UPDATE_FRACTION: u128 = 11_684_671;

const MIN_BASE_FEE_PER_BLOB_GAS: u128 = 1;

/// Approximation of `factor * e ** (numerator / denominator)` via Taylor expansion.
fn fake_exponential(factor: u128, numerator: u128, denominator: u128) -> u128 {
    let mut i = 1u128;
    let mut output = 0u128;
    let mut numerator_accum = factor * denominator;
    while numerator_accum > 0 {
        output += numerator_accum;
        numerator_accum = (numerator_accum * numerator) / (denominator * i);
        i += 1;
    }
    output / denominator
}

impl Block {
    pub fn get_block_context(&self) -> rig::BlockContext {
        let base_fee = U256::from(self.result.header.base_fee_per_gas.unwrap_or(1000));
        // Compute the blob base fee with the fork-appropriate update fraction
        // (alloy's `blob_fee()` does not track the BPO schedule).
        let blob_fee = self
            .result
            .header
            .excess_blob_gas
            .map(|ebg| {
                U256::from(fake_exponential(
                    MIN_BASE_FEE_PER_BLOB_GAS,
                    ebg as u128,
                    BLOB_BASE_FEE_UPDATE_FRACTION,
                ))
            })
            .unwrap_or(U256::MAX);
        rig::BlockContext {
            timestamp: self.result.header.timestamp,
            eip1559_basefee: base_fee,
            pubdata_price: U256::ZERO,
            native_price: U256::ZERO,
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
                    // Supported: legacy(0), 2930(1), 1559(2), 4844(3), 7702(4).
                    let supported_tx_type = transaction_type <= 4;
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
