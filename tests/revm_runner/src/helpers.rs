use alloy::{consensus::Transaction, eips::Typed2718, primitives::TxKind};
use anyhow::{anyhow, bail, Context};
use reth_revm::context::TxEnv;
use zksync_os_revm::{transaction::abstraction::ZKsyncTxBuilder, ZKsyncTx};
use zksync_os_tests_common::zksync_tx::{ZKsyncSpecificTxEnvelope, ZKsyncTxEnvelope};

fn checked_u64(value: u128, field: &str) -> anyhow::Result<u64> {
    u64::try_from(value).with_context(|| format!("{field} does not fit into u64: {value}"))
}

/// Convert a ZkTransaction into a revm TxEnv for REVM re-execution
pub fn zk_tx_into_revm_tx(
    tx: &ZKsyncTxEnvelope,
    gas_used_override: Option<u64>,
    force_revert: bool,
) -> anyhow::Result<ZKsyncTx<TxEnv>> {
    let (
        gas_price,
        gas_priority_fee,
        value,
        data,
        chain_id,
        access_list,
        to_mint,
        refund_recipient,
        caller,
        gas_limit,
        nonce,
    ) = match &tx {
        ZKsyncTxEnvelope::Ethereum(ethereum_tx_envelope, signer) => {
            // L2 transactions are standard Ethereum transactions
            let gas_price = Some(ethereum_tx_envelope.max_fee_per_gas());
            let priority_fee = ethereum_tx_envelope.max_priority_fee_per_gas();
            let value = Some(ethereum_tx_envelope.value());
            let data = ethereum_tx_envelope.input().clone();
            let chain_id = ethereum_tx_envelope.chain_id();
            let access_list = ethereum_tx_envelope
                .access_list()
                .cloned()
                .unwrap_or_default();
            let gas_limit = ethereum_tx_envelope.gas_limit();
            let nonce = ethereum_tx_envelope.nonce();

            (
                gas_price,
                priority_fee,
                value,
                data,
                chain_id,
                access_list,
                Default::default(),
                None,
                *signer,
                gas_limit,
                nonce,
            )
        }
        ZKsyncTxEnvelope::ZKsync(zksync_specific_tx_envelope) => {
            match zksync_specific_tx_envelope {
                ZKsyncSpecificTxEnvelope::L1(zksync_l1_tx) => {
                    let gas_limit = checked_u64(zksync_l1_tx.gas_limit, "L1 tx gas_limit")?;
                    let nonce = checked_u64(zksync_l1_tx.nonce, "L1 tx nonce")?;
                    (
                        Some(zksync_l1_tx.max_fee_per_gas),
                        Some(zksync_l1_tx.max_priority_fee_per_gas),
                        Some(zksync_l1_tx.value),
                        zksync_l1_tx.input.clone(),
                        None, // Chain id is not specified in ZKsync specific transactions
                        Default::default(), // L1 transactions don't have access lists
                        zksync_l1_tx.to_mint,
                        Some(zksync_l1_tx.refund_recipient),
                        zksync_l1_tx.from,
                        gas_limit,
                        nonce,
                    )
                }
                ZKsyncSpecificTxEnvelope::Upgrade(zksync_upgrade_tx) => {
                    let gas_limit =
                        checked_u64(zksync_upgrade_tx.gas_limit, "Upgrade tx gas_limit")?;
                    let nonce = checked_u64(zksync_upgrade_tx.nonce, "Upgrade tx nonce")?;
                    (
                        Some(zksync_upgrade_tx.max_fee_per_gas),
                        Some(zksync_upgrade_tx.max_priority_fee_per_gas),
                        Some(zksync_upgrade_tx.value),
                        zksync_upgrade_tx.input.clone(),
                        None, // Chain id is not specified in ZKsync specific transactions
                        Default::default(), // L1 transactions don't have access lists
                        zksync_upgrade_tx.to_mint,
                        Some(zksync_upgrade_tx.refund_recipient),
                        zksync_upgrade_tx.from,
                        gas_limit,
                        nonce,
                    )
                }
                ZKsyncSpecificTxEnvelope::Service(_) => {
                    bail!("System transactions are not supported by REVM runner");
                }
            }
        }
        ZKsyncTxEnvelope::Custom(_, _) => {
            bail!("Custom transactions are not supported by REVM runner");
        }
    };

    // Determine transaction kind (Call or Create)
    let transact_to = match tx.to() {
        Some(to) => TxKind::Call(to),
        None => TxKind::Create,
    };

    // Build TxEnv using the builder pattern
    let mut tx_env_builder = TxEnv::builder()
        .caller(caller)
        .gas_limit(gas_limit)
        .gas_price(gas_price.unwrap_or_default())
        .kind(transact_to)
        .value(value.unwrap_or_default())
        .data(data)
        .nonce(nonce)
        .access_list(access_list)
        .tx_type(Some(tx.ty()))
        .chain_id(chain_id)
        .blob_hashes(vec![]); // ZkSync transactions don't use blobs yet

    if let Some(priority_fee) = gas_priority_fee {
        tx_env_builder = tx_env_builder.gas_priority_fee(Some(priority_fee));
    }

    ZKsyncTxBuilder::new()
        .base(tx_env_builder)
        .mint(to_mint)
        .refund_recipient(refund_recipient)
        .gas_used_override(gas_used_override)
        .force_fail(force_revert)
        .build()
        .map_err(|e| anyhow!("Failed to build TxEnv: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::rpc::types::TransactionRequest;
    use zksync_os_tests_common::zksync_tx::{l1_tx::ZKsyncL1Tx, service_tx::ZKsyncServiceTx};

    #[test]
    fn custom_tx_is_rejected() {
        let tx = ZKsyncTxEnvelope::new_custom_tx_type(TransactionRequest::default(), 0xff);
        let err = zk_tx_into_revm_tx(&tx, None, false).unwrap_err();
        assert!(err.to_string().contains("Custom transactions"));
    }

    #[test]
    fn service_tx_is_rejected() {
        let service_tx = ZKsyncServiceTx::default();
        let tx = ZKsyncTxEnvelope::from(service_tx);
        let err = zk_tx_into_revm_tx(&tx, None, false).unwrap_err();
        assert!(err.to_string().contains("System transactions"));
    }

    #[test]
    fn overflowing_l1_gas_limit_is_rejected() {
        let tx = ZKsyncTxEnvelope::from(ZKsyncL1Tx {
            gas_limit: (u64::MAX as u128) + 1,
            ..Default::default()
        });
        let err = zk_tx_into_revm_tx(&tx, None, false).unwrap_err();
        assert!(err.to_string().contains("gas_limit"));
    }
}
