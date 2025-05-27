//!
//! These tests are focused on different tx types, AA features.
//!
#![cfg(test)]

use alloy::primitives::TxKind;
use rig::alloy::primitives::address;
use rig::alloy::rpc::types::TransactionRequest;
use rig::forward_system::run::ExecutionResult;
use rig::ruint::aliases::B160;
use rig::utils::{address_into_special_storage_key, ACCOUNT_PROPERTIES_STORAGE_ADDRESS};
use rig::{alloy, Chain};

#[test]
fn test_set_deployed_bytecode_evm() {
    let mut chain = Chain::empty(None);

    let l2_genesis_upgrade_address = address!("0000000000000000000000000000000000010001");
    let contract_deployer_address = address!("0000000000000000000000000000000000008006");
    let calldata =
        hex::decode("1223adc70000000000000000000000000000000000000000000000000000000000010002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000050123456789000000000000000000000000000000000000000000000000000000")
            .unwrap();

    let encdoed_tx = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(l2_genesis_upgrade_address),
            to: Some(TxKind::Call(contract_deployer_address)),
            input: calldata.into(),
            gas: Some(200_000),
            max_fee_per_gas: Some(1000),
            max_priority_fee_per_gas: Some(1000),
            value: Some(alloy::primitives::U256::from(0)),
            nonce: Some(0),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(tx)
    };
    let transactions = vec![encdoed_tx];

    let output = chain.run_block(transactions, None, None);

    // Assert all txs succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {} failed with: {:?}", i, r)
        }
        success
    }));

    let expected_account_hash =
        rig::utils::evm_bytecode_into_account_properties(&[0x01, 0x23, 0x45, 0x67, 0x89])
            .compute_hash();
    let actual_hash = output
        .storage_writes
        .iter()
        .find(|write| {
            write.account == ACCOUNT_PROPERTIES_STORAGE_ADDRESS
                && write.account_key
                    == address_into_special_storage_key(&B160::from_limbs([0x10002, 0, 0]))
        })
        .expect("Corresponding write for force deploy not found")
        .value;

    assert_eq!(expected_account_hash, actual_hash);
}

#[test]
fn test_set_deployed_bytecode_evm_unauthorized() {
    let mut chain = Chain::empty(None);

    let from = address!("0000000000000000000000000000000000010003");
    let contract_deployer_address = address!("0000000000000000000000000000000000008006");
    let calldata =
        hex::decode("1223adc70000000000000000000000000000000000000000000000000000000000010002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000050123456789000000000000000000000000000000000000000000000000000000")
            .unwrap();

    let encdoed_tx = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(from),
            to: Some(TxKind::Call(contract_deployer_address)),
            input: calldata.into(),
            gas: Some(200_000),
            max_fee_per_gas: Some(1000),
            max_priority_fee_per_gas: Some(1000),
            value: Some(alloy::primitives::U256::from(0)),
            nonce: Some(0),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(tx)
    };
    let transactions = vec![encdoed_tx];

    let output = chain.run_block(transactions, None, None);

    if let ExecutionResult::Success(_) = output
        .tx_results
        .first()
        .unwrap()
        .as_ref()
        .unwrap()
        .execution_result
    {
        panic!("force deploy from unauthorized sender haven't failed")
    }
}
