use alloy::{
    consensus::{Transaction, Typed2718},
    primitives::Bytes,
    rpc::types::Transaction as RpcTransaction,
};
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use reth_revm::{context::TxEnv, state::Bytecode};
use zksync_os_revm::{transaction::abstraction::ZKsyncTxBuilder, ZKsyncTx};

/// Get unpadded code from full bytecode with artifacts.
pub fn get_unpadded_code(full_bytecode: &[u8], account: &AccountProperties) -> Bytecode {
    Bytecode::new_legacy(Bytes::copy_from_slice(
        &full_bytecode[0..account.unpadded_code_len as usize],
    ))
}

/// Convert a ZkTransaction into a revm TxEnv for REVM re-execution
pub fn zk_tx_into_revm_tx(
    tx: &RpcTransaction,
    gas_used_override: Option<u64>,
    force_revert: bool,
) -> ZKsyncTx<TxEnv> {
    let tx_inner = tx.as_recovered();

    // Build TxEnv using the builder pattern
    let mut tx_env_builder = TxEnv::builder()
        .caller(tx_inner.signer())
        .gas_limit(tx.gas_limit())
        .gas_price(tx.max_fee_per_gas())
        .kind(tx.kind())
        .value(tx.value())
        .data(tx.input().clone())
        .nonce(tx.nonce())
        .access_list(tx.access_list().cloned().unwrap_or_default())
        .tx_type(Some(tx.ty()))
        .chain_id(tx.chain_id())
        .blob_hashes(
            tx.blob_versioned_hashes()
                .map_or_else(Vec::new, |hashes| hashes.to_vec()),
        )
        .max_fee_per_blob_gas(tx.max_fee_per_blob_gas().unwrap_or_default());

    if let Some(priority_fee) = tx.max_priority_fee_per_gas() {
        tx_env_builder = tx_env_builder.gas_priority_fee(Some(priority_fee));
    }

    if let Some(authorizations) = tx.authorization_list() {
        tx_env_builder = tx_env_builder.authorization_list_signed(authorizations.to_vec());
    }

    ZKsyncTxBuilder::new()
        .base(tx_env_builder)
        .gas_used_override(gas_used_override)
        .force_fail(force_revert)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build TxEnv: {e:?}"))
        .unwrap()
}
