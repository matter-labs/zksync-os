use alloy::{hex, primitives::U256};
use anyhow::Result;
use rig::zksync_os_interface::types::{BlockOutput, TxOutput};

use crate::rpc_client::{Block, TransactionReceipt};

pub fn filter_supported_receipts(
    block: &Block,
    receipts: Vec<TransactionReceipt>,
) -> Result<Vec<TransactionReceipt>> {
    let block_tx_count = block.tx_count();
    if block_tx_count != receipts.len() {
        return Err(anyhow::anyhow!(
            "receipt count mismatch: block has {} txs, RPC returned {} receipts",
            block_tx_count,
            receipts.len()
        ));
    }

    Ok(receipts)
}

pub fn check_tx_outputs_against_receipts(
    block_output: &BlockOutput,
    receipts: &[TransactionReceipt],
) -> Result<()> {
    if block_output.tx_results.len() != receipts.len() {
        return Err(anyhow::anyhow!(
            "tx result count mismatch: got {} results, {} receipts",
            block_output.tx_results.len(),
            receipts.len()
        ));
    }

    for (idx, (res, receipt)) in block_output
        .tx_results
        .iter()
        .zip(receipts.iter())
        .enumerate()
    {
        let res = res.as_ref().map_err(|err| {
            anyhow::anyhow!(
                "tx #{idx} (hash={}) is invalid in block output: {err:?}",
                receipt.transaction_hash
            )
        })?;
        check_single_tx_against_receipt(idx, res, receipt)?;
    }

    Ok(())
}

pub fn check_selected_tx_outputs_against_receipts(
    block_output: &BlockOutput,
    tx_idx_to_receipt: &[(usize, TransactionReceipt)],
) -> Result<()> {
    for (idx, receipt) in tx_idx_to_receipt {
        let Some(res) = block_output.tx_results.get(*idx) else {
            return Err(anyhow::anyhow!(
                "tx #{idx} (hash={}) is out of bounds for block output with {} tx results",
                receipt.transaction_hash,
                block_output.tx_results.len()
            ));
        };

        let res = res.as_ref().map_err(|err| {
            anyhow::anyhow!(
                "tx #{idx} (hash={}) is invalid in block output: {err:?}",
                receipt.transaction_hash
            )
        })?;
        check_single_tx_against_receipt(*idx, res, receipt)?;
    }

    Ok(())
}

fn check_single_tx_against_receipt(
    idx: usize,
    res: &TxOutput,
    receipt: &TransactionReceipt,
) -> Result<()> {
    let expected_success = match receipt.status {
        Some(v) if v == U256::ONE => Some(true),
        Some(v) if v == U256::ZERO => Some(false),
        Some(v) => {
            return Err(anyhow::anyhow!(
                "tx #{idx} (hash={}) has unexpected receipt status {}",
                receipt.transaction_hash,
                v
            ));
        }
        None => None,
    };

    if let Some(expected_success) = expected_success {
        if res.is_success() != expected_success {
            return Err(anyhow::anyhow!(
                "tx #{idx} (hash={}) status mismatch: output_success={} receipt_success={}",
                receipt.transaction_hash,
                res.is_success(),
                expected_success
            ));
        }
    }

    let expected_gas = rig::zk_ee::utils::u256_to_u64_saturated(&receipt.gas_used);
    if res.gas_used != expected_gas {
        return Err(anyhow::anyhow!(
            "tx #{idx} (hash={}) gas mismatch: output={} receipt={}",
            receipt.transaction_hash,
            res.gas_used,
            expected_gas
        ));
    }

    if res.logs.len() != receipt.logs.len() {
        return Err(anyhow::anyhow!(
            "tx #{idx} (hash={}) log count mismatch: output={} receipt={}",
            receipt.transaction_hash,
            res.logs.len(),
            receipt.logs.len()
        ));
    }

    for (log_idx, (actual_log, receipt_log)) in res.logs.iter().zip(receipt.logs.iter()).enumerate()
    {
        if !receipt_log.is_equal_to_excluding_data(actual_log) {
            return Err(anyhow::anyhow!(
                "tx #{idx} (hash={}) log #{log_idx} metadata mismatch",
                receipt.transaction_hash
            ));
        }
        if receipt_log.data.as_ref() != actual_log.data.data.as_ref() {
            return Err(anyhow::anyhow!(
                "tx #{idx} (hash={}) log #{log_idx} data mismatch: output=0x{} receipt=0x{}",
                receipt.transaction_hash,
                hex::encode(actual_log.data.data.clone()),
                hex::encode(receipt_log.data.as_ref())
            ));
        }
    }

    Ok(())
}
