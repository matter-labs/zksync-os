use alloy::{
    consensus::Transaction,
    primitives::{Bytes, TxKind},
    rpc::types::TransactionInput,
};
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use reth_revm::{context::TxEnv, state::Bytecode};
use zksync_os_revm::{transaction::abstraction::ZKsyncTxBuilder, ZKsyncTx};
use zksync_os_tests_common::zksync_tx::{ZKsyncSpecificTxEnvelope, ZKsyncTxEnvelope};

/// Get unpadded code from full bytecode with artifacts.
pub fn get_unpadded_code(full_bytecode: &[u8], account: &AccountProperties) -> Bytecode {
    Bytecode::new_legacy(Bytes::copy_from_slice(
        &full_bytecode[0..account.unpadded_code_len as usize],
    ))
}

/// Convert a ZkTransaction into a revm TxEnv for REVM re-execution
pub fn zk_tx_into_revm_tx(
    tx: &ZKsyncTxEnvelope,
    gas_used_override: Option<u64>,
    force_revert: bool,
) -> ZKsyncTx<TxEnv> {
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
        gas,
        nonce,
    ) = match &tx {
        ZKsyncTxEnvelope::Ethereum(ethereum_tx_envelope, signer) => {
            // L2 transactions are standard Ethereum transactions
            let gas_price = Some(ethereum_tx_envelope.max_fee_per_gas());
            let priority_fee = ethereum_tx_envelope.max_priority_fee_per_gas();
            let value = Some(ethereum_tx_envelope.value());
            let input = ethereum_tx_envelope.input();
            let chain_id = ethereum_tx_envelope.chain_id();
            let access_list = ethereum_tx_envelope
                .access_list()
                .cloned()
                .unwrap_or_default();
            let gas = Some(ethereum_tx_envelope.gas_limit());
            let nonce = Some(ethereum_tx_envelope.nonce());

            (
                gas_price,
                priority_fee,
                value,
                TransactionInput::new(input.clone()),
                chain_id,
                access_list,
                Default::default(),
                None,
                signer.address(),
                gas,
                nonce,
            )
        }
        ZKsyncTxEnvelope::ZKsyncEnvelope(zksync_specific_tx_envelope) => {
            match zksync_specific_tx_envelope {
                ZKsyncSpecificTxEnvelope::L1(zksync_l1_tx) => {
                    (
                        Some(zksync_l1_tx.max_fee_per_gas),
                        Some(zksync_l1_tx.max_priority_fee_per_gas),
                        Some(zksync_l1_tx.value),
                        TransactionInput::new(zksync_l1_tx.input.clone()),
                        None, // Chain id is not specified in ZKsync specific transactions
                        Default::default(), // L1 transactions don't have access lists
                        zksync_l1_tx.to_mint,
                        Some(zksync_l1_tx.refund_recipient),
                        zksync_l1_tx.from,
                        Some(zksync_l1_tx.gas_limit.try_into().unwrap()), // TODO conversion
                        Some(zksync_l1_tx.nonce.try_into().unwrap()),     // TODO conversion
                    )
                }
                ZKsyncSpecificTxEnvelope::Upgrade(zksync_upgrade_tx) => {
                    (
                        Some(zksync_upgrade_tx.max_fee_per_gas),
                        Some(zksync_upgrade_tx.max_priority_fee_per_gas),
                        Some(zksync_upgrade_tx.value),
                        TransactionInput::new(zksync_upgrade_tx.input.clone()),
                        None, // Chain id is not specified in ZKsync specific transactions
                        Default::default(), // L1 transactions don't have access lists
                        zksync_upgrade_tx.to_mint,
                        Some(zksync_upgrade_tx.refund_recipient),
                        zksync_upgrade_tx.from,
                        Some(zksync_upgrade_tx.gas_limit.try_into().unwrap()), // TODO conversion
                        Some(zksync_upgrade_tx.nonce.try_into().unwrap()),     // TODO conversion
                    )
                }
                ZKsyncSpecificTxEnvelope::Service(_) => {
                    unimplemented!(
                        "System transactions are not currently supported by REVM runner"
                    );
                }
            }
        }
        ZKsyncTxEnvelope::Custom(_, _) => {
            panic!("Custom transactions are not supported by REVM runner")
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
        .gas_limit(gas.unwrap())
        .gas_price(gas_price.unwrap_or_default())
        .kind(transact_to)
        .value(value.unwrap_or_default())
        .data(data.input.unwrap_or_default())
        .nonce(nonce.unwrap())
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
        .map_err(|e| anyhow::anyhow!("Failed to build TxEnv: {e:?}"))
        .unwrap()
}
