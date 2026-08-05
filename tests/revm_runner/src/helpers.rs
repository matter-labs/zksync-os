use alloy::{
    consensus::Transaction,
    eips::{
        eip4844::{fake_exponential, BLOB_GASPRICE_UPDATE_FRACTION, BLOB_TX_MIN_BLOB_GASPRICE},
        Typed2718,
    },
    primitives::{TxKind, U256},
};
use anyhow::{anyhow, bail, Context};
use revm::context::TxEnv;
use zksync_os_revm::{transaction::abstraction::ZKsyncTxBuilder, ZKsyncTx};
use zksync_os_tests_common::zksync_tx::{
    encoding::BOOTLOADER_FORMAL_ADDRESS, ZKsyncSpecificTxEnvelope, ZKsyncTxEnvelope,
};

fn checked_u64(value: u128, field: &str) -> anyhow::Result<u64> {
    u64::try_from(value).with_context(|| format!("{field} does not fit into u64: {value}"))
}

/// Convert a ZkTransaction into a revm TxEnv for REVM re-execution
pub fn zk_tx_into_revm_tx(
    tx: &ZKsyncTxEnvelope,
    gas_used_override: Option<u64>,
    force_revert: bool,
    block_gas_limit: u64,
    settlement_layer_chain_id: Option<U256>,
) -> anyhow::Result<ZKsyncTx<TxEnv>> {
    let mut blob_hashes = vec![];
    let mut max_fee_per_blob_gas = 0;
    let mut authorization_list = vec![];
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
            blob_hashes = ethereum_tx_envelope
                .blob_versioned_hashes()
                .map(|hashes| hashes.to_vec())
                .unwrap_or_default();
            max_fee_per_blob_gas = ethereum_tx_envelope
                .max_fee_per_blob_gas()
                .unwrap_or_default();

            authorization_list = ethereum_tx_envelope
                .authorization_list()
                .map(|list| list.to_vec())
                .unwrap_or_default();

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
                ZKsyncSpecificTxEnvelope::Service(service_tx) => {
                    let gas_limit = block_gas_limit;
                    let nonce = 0; // Service transactions don't have nonces, use neutral placeholder.
                    (
                        Some(0u128),
                        Some(0u128),
                        Some(U256::ZERO),
                        service_tx.input.clone(),
                        None, // Chain id is not specified in ZKsync specific transactions
                        Default::default(), // Service transactions don't have access lists
                        Default::default(), // Service transactions don't mint
                        None, // Service transactions don't have refund recipients
                        BOOTLOADER_FORMAL_ADDRESS,
                        gas_limit,
                        nonce,
                    )
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
        .blob_hashes(blob_hashes)
        .max_fee_per_blob_gas(max_fee_per_blob_gas)
        .authorization_list_signed(authorization_list);

    if let Some(priority_fee) = gas_priority_fee {
        tx_env_builder = tx_env_builder.gas_priority_fee(Some(priority_fee));
    }

    ZKsyncTxBuilder::new()
        .base(tx_env_builder)
        .mint(to_mint)
        .refund_recipient(refund_recipient)
        .settlement_layer_chain_id(settlement_layer_chain_id)
        .gas_used_override(gas_used_override)
        .force_fail(force_revert)
        .build()
        .map_err(|e| anyhow!("Failed to build TxEnv: {e:?}"))
}

pub const BLOB_BASE_FEE_UPDATE_FRACTION: u128 = BLOB_GASPRICE_UPDATE_FRACTION;
pub const MIN_BASE_FEE_PER_BLOB_GAS: u64 = 1;

pub fn calculate_excess_blob_gas_from_blob_base_fee(
    blob_base_fee: u64,
    blob_base_fee_update_fraction: u128,
) -> u64 {
    if blob_base_fee <= MIN_BASE_FEE_PER_BLOB_GAS {
        return 0;
    }
    assert!(
        blob_base_fee_update_fraction != 0,
        "blob base fee update fraction cannot be zero"
    );

    let target_blob_base_fee = blob_base_fee as u128;
    let mut low = 0u64;
    let mut high = 1u64;

    while calculate_blob_base_fee_for_excess_blob_gas(high, blob_base_fee_update_fraction)
        < target_blob_base_fee
    {
        if high == u64::MAX {
            return u64::MAX;
        }
        high = high.saturating_mul(2);
    }

    while low < high {
        let mid = low + (high - low) / 2;
        let blob_base_fee_at_mid =
            calculate_blob_base_fee_for_excess_blob_gas(mid, blob_base_fee_update_fraction);
        if blob_base_fee_at_mid < target_blob_base_fee {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    low
}

fn calculate_blob_base_fee_for_excess_blob_gas(
    excess_blob_gas: u64,
    blob_base_fee_update_fraction: u128,
) -> u128 {
    fake_exponential(
        BLOB_TX_MIN_BLOB_GASPRICE,
        excess_blob_gas as u128,
        blob_base_fee_update_fraction,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::rpc::types::TransactionRequest;
    use zksync_os_tests_common::zksync_tx::l1_tx::ZKsyncL1Tx;

    #[test]
    fn custom_tx_is_rejected() {
        let tx = ZKsyncTxEnvelope::new_custom_tx_type(TransactionRequest::default(), 0xff);
        let err = zk_tx_into_revm_tx(&tx, None, false, 30_000_000, None).unwrap_err();
        assert!(err.to_string().contains("Custom transactions"));
    }

    #[test]
    fn overflowing_l1_gas_limit_is_rejected() {
        let tx = ZKsyncTxEnvelope::from(ZKsyncL1Tx {
            gas_limit: (u64::MAX as u128) + 1,
            ..Default::default()
        });
        let err = zk_tx_into_revm_tx(&tx, None, false, 30_000_000, None).unwrap_err();
        assert!(err.to_string().contains("gas_limit"));
    }

    #[test]
    fn zero_blob_base_fee_maps_to_zero_excess_blob_gas() {
        assert_eq!(
            calculate_excess_blob_gas_from_blob_base_fee(0, BLOB_BASE_FEE_UPDATE_FRACTION),
            0
        );
    }

    #[test]
    fn excess_blob_gas_inverse_returns_minimum_matching_value() {
        let test_cases = [0u64, 1, 2, 100_000, 2_314_058, 10_000_000];
        for excess_blob_gas in test_cases {
            let blob_base_fee = calculate_blob_base_fee_for_excess_blob_gas(
                excess_blob_gas,
                BLOB_BASE_FEE_UPDATE_FRACTION,
            );
            let blob_base_fee_u64: u64 = blob_base_fee
                .try_into()
                .expect("test vector should fit into u64");

            let recovered_excess_blob_gas = calculate_excess_blob_gas_from_blob_base_fee(
                blob_base_fee_u64,
                BLOB_BASE_FEE_UPDATE_FRACTION,
            );

            let recovered_blob_base_fee = calculate_blob_base_fee_for_excess_blob_gas(
                recovered_excess_blob_gas,
                BLOB_BASE_FEE_UPDATE_FRACTION,
            );
            assert!(recovered_blob_base_fee >= blob_base_fee);

            if recovered_excess_blob_gas > 0 {
                let previous_blob_base_fee = calculate_blob_base_fee_for_excess_blob_gas(
                    recovered_excess_blob_gas - 1,
                    BLOB_BASE_FEE_UPDATE_FRACTION,
                );
                assert!(previous_blob_base_fee < blob_base_fee);
            }
        }
    }
}
