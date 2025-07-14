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
use rig::zk_ee::utils::Bytes32;
use rig::{alloy, Chain};

#[test]
fn test_set_bytecode_details_evm() {
    let mut chain = Chain::empty(None);

    let complex_upgrader_address = address!("000000000000000000000000000000000000800f");
    let contract_deployer_address = address!("0000000000000000000000000000000000008006");
    // setBytecodeDetailsEVM(address,bytes32,uint32,bytes32) - f6eca0b0
    let bytecode = hex::decode("0123456789").unwrap();
    let code_hash = Bytes32::from_array(
        hex::decode("1c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    chain.set_preimage(code_hash, &bytecode);
    let calldata =
        hex::decode("f6eca0b000000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b944")
            .unwrap();

    let encdoed_tx = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(complex_upgrader_address),
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
            .0
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

    let from = address!("000000000000000000000000000000000000800e");
    let contract_deployer_address = address!("0000000000000000000000000000000000008006");
    let calldata =
        hex::decode("f6eca0b000000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b944")
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
