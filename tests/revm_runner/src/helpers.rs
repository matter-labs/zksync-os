use alloy::primitives::{Bytes, TxKind, U256};
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use reth_revm::{context::TxEnv, state::Bytecode};
use zksync_os_revm::{transaction::abstraction::ZKsyncTxBuilder, ZKsyncTx};
use zksync_os_tests_common::zksync_tx::{ZKsyncTxRequest, ZKsyncTxType};

/// Get unpadded code from full bytecode with artifacts.
pub fn get_unpadded_code(full_bytecode: &[u8], account: &AccountProperties) -> Bytecode {
    Bytecode::new_legacy(Bytes::copy_from_slice(
        &full_bytecode[0..account.unpadded_code_len as usize],
    ))
}

/// Convert a ZkTransaction into a revm TxEnv for REVM re-execution
pub fn zk_tx_into_revm_tx(
    tx: &ZKsyncTxRequest,
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
    ) = match tx.tx_type {
        ZKsyncTxType::Service => {
            unimplemented!("handle system txs");
        }
        ZKsyncTxType::L2 => {
            // L2 transactions are standard Ethereum transactions
            let gas_price = tx.inner.max_fee_per_gas;
            let priority_fee = tx.inner.max_priority_fee_per_gas;
            let value = tx.inner.value;
            let data = tx.inner.input.clone();
            let chain_id = tx.inner.chain_id;
            let access_list = tx.inner.access_list.clone().unwrap_or_default();

            (
                gas_price,
                priority_fee,
                value,
                data,
                chain_id,
                access_list,
                Default::default(),
                None,
                tx.signer
                    .as_ref()
                    .expect("L2 tx must have a signer")
                    .address(),
            )
        }
        ZKsyncTxType::L1 => {
            // L1 priority transactions - extract from canonical transaction format
            let inner = &tx.inner;
            (
                inner.max_fee_per_gas,
                inner.max_priority_fee_per_gas,
                inner.value,
                inner.input.clone(),
                None,
                Default::default(), // L1 transactions don't have access lists
                U256::ZERO,         // TODO: Minting is not supported for l1 transactions in our rig
                None,               // TODO: Minting is not supported for l1 transactions in our rig
                inner.from.expect("L1 tx should have from field"),
            )
        }
        ZKsyncTxType::Upgrade => {
            // Upgrade transactions - system-level transactions
            let inner = &tx.inner;
            (
                None,
                None,
                inner.value,
                inner.input.clone(),
                None,
                Default::default(),
                U256::ZERO, // TODO: Minting is not supported for upgrade transactions in our rig
                None,       // TODO: Minting is not supported for upgrade transactions in our rig
                inner.from.expect("L1 tx should have from field"), // TODO check it
            )
        }
    };

    // Determine transaction kind (Call or Create)
    let transact_to = match tx.inner.to {
        Some(to) => to,
        None => TxKind::Create,
    };

    // Build TxEnv using the builder pattern
    let mut tx_env_builder = TxEnv::builder()
        .caller(caller)
        .gas_limit(tx.inner.gas.unwrap_or_default())
        .gas_price(gas_price.unwrap_or_default())
        .kind(transact_to)
        .value(value.unwrap_or_default())
        .data(data.input.unwrap_or_default())
        .nonce(tx.inner.nonce.unwrap_or_default())
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
