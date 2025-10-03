//!
//! These tests are focused on different tx types.
//!
#![cfg(test)]

use alloy::primitives::TxKind;
use alloy_sol_types::{sol, SolEvent};
use rig::alloy::primitives::address;
use rig::alloy::rpc::types::TransactionRequest;
use rig::ruint::aliases::{B160, U256};
use rig::utils::{
    address_into_special_storage_key, AccountProperties, ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use rig::zk_ee::utils::Bytes32;
use rig::zksync_os_interface::types::ExecutionResult;
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

    let encoded_tx = {
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
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None);

    // Assert all txs succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {} failed with: {:?}", i, r)
        }
        success
    }));

    let mut account = AccountProperties::default();
    rig::zksync_os_api::helpers::set_properties_code(&mut account, &[0x01, 0x23, 0x45, 0x67, 0x89]);
    let expected_account_hash = account.compute_hash();

    let actual_hash = output
        .storage_writes
        .iter()
        .find(|write| {
            write.account.0 == ACCOUNT_PROPERTIES_STORAGE_ADDRESS.to_be_bytes()
                && write.account_key.0
                    == address_into_special_storage_key(&B160::from_limbs([0x10002, 0, 0]))
                        .as_u8_array()
        })
        .expect("Corresponding write for force deploy not found")
        .value;

    assert_eq!(expected_account_hash.as_u8_array(), actual_hash.0);
}

#[test]
fn test_set_deployed_bytecode_evm_unauthorized() {
    let mut chain = Chain::empty(None);

    let from = address!("000000000000000000000000000000000000800e");
    let contract_deployer_address = address!("0000000000000000000000000000000000008006");
    let calldata =
        hex::decode("f6eca0b000000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b944")
            .unwrap();

    let encoded_tx = {
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
    let transactions = vec![encoded_tx];

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

#[test]
fn test_l2_base_token_withdraw_events() {
    let mut chain = Chain::empty(None);

    // L2 base token address is 0x800a
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(1000000000000000000u64); // 1 ETH

    chain.set_balance(B160::from_be_bytes(sender.into_array()), withdrawal_amount);

    // Prepare withdraw(address) calldata
    // withdraw(address) has selector 0x51cff8d9
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("51cff8d9").unwrap()); // withdraw selector
    calldata.extend_from_slice(&[0u8; 12]); // padding for address
    calldata.extend_from_slice(l1_receiver.as_slice()); // l1_receiver address

    let tx = TransactionRequest {
        chain_id: Some(37),
        from: Some(sender),
        to: Some(TxKind::Call(l2_base_token_address)),
        input: calldata.into(),
        value: Some(withdrawal_amount),
        gas: Some(200_000),
        max_fee_per_gas: Some(1000),
        max_priority_fee_per_gas: Some(1000),
        nonce: Some(0),
        ..TransactionRequest::default()
    };

    let encoded_tx = rig::utils::encode_l1_tx(tx);
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None);

    // Assert transaction succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {} failed with: {:?}", i, r)
        }
        success
    }));

    sol! {
        event Withdrawal(address indexed _l2Sender, address indexed _l1Receiver, uint256 _amount);
    }

    // Check that withdrawal with message event was emitted
    let withdrawal_event = output.tx_results[0]
        .as_ref()
        .unwrap()
        .logs
        .iter()
        .find(|event| Withdrawal::decode_log_data(&event).is_ok());
    assert!(
        withdrawal_event.is_some(),
        "Withdrawal event should be emitted"
    );

    let event = Withdrawal::decode_log_data(withdrawal_event.unwrap()).unwrap();

    // Verify event fields
    assert_eq!(event._l2Sender.as_slice(), sender.0.as_slice());
    assert_eq!(event._l1Receiver.as_slice(), l1_receiver.0.as_slice());
    assert_eq!(event._amount, withdrawal_amount);
}

#[test]
fn test_l2_base_token_withdraw_with_message_events() {
    let mut chain = Chain::empty(None);

    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH
    let additional_data = b"test message data";

    // Set up initial balance for the sender
    chain.set_balance(B160::from_be_bytes(sender.into_array()), withdrawal_amount);

    // Prepare withdrawWithMessage(address,bytes) calldata
    // withdrawWithMessage(address,bytes) has selector 0x84bc3eb0
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("84bc3eb0").unwrap()); // withdrawWithMessage selector
    calldata.extend_from_slice(&[0u8; 12]); // padding for address
    calldata.extend_from_slice(l1_receiver.as_slice()); // l1_receiver address

    // Offset to the bytes data (0x40 = 64)
    calldata.extend_from_slice(&[0u8; 31]);
    calldata.push(0x40);

    // Length of additional data
    calldata.extend_from_slice(&[0u8; 31]);
    calldata.push(additional_data.len() as u8);

    // Additional data, padded to 32 bytes
    calldata.extend_from_slice(additional_data);
    let padding_needed = 32 - (additional_data.len() % 32);
    if padding_needed != 32 {
        calldata.extend_from_slice(&vec![0u8; padding_needed]);
    }

    let tx = TransactionRequest {
        chain_id: Some(37),
        from: Some(sender),
        to: Some(TxKind::Call(l2_base_token_address)),
        input: calldata.into(),
        value: Some(withdrawal_amount),
        gas: Some(300_000),
        max_fee_per_gas: Some(1000),
        max_priority_fee_per_gas: Some(1000),
        nonce: Some(0),
        ..TransactionRequest::default()
    };

    let encoded_tx = rig::utils::encode_l1_tx(tx);
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None);

    // Assert transaction succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {} failed with: {:?}", i, r)
        }
        success
    }));

    sol! {
        event WithdrawalWithMessage(address indexed _l2Sender, address indexed _l1Receiver, uint256 _amount, bytes _additionalData);
    }

    // Check that withdrawal with message event was emitted
    let withdrawal_event = output.tx_results[0]
        .as_ref()
        .unwrap()
        .logs
        .iter()
        .find(|event| WithdrawalWithMessage::decode_log_data(&event).is_ok());
    assert!(
        withdrawal_event.is_some(),
        "WithdrawalWithMessage event should be emitted"
    );

    let event = WithdrawalWithMessage::decode_log_data(withdrawal_event.unwrap()).unwrap();

    // Verify event fields
    assert_eq!(event._l2Sender.as_slice(), sender.0.as_slice());
    assert_eq!(event._l1Receiver.as_slice(), l1_receiver.0.as_slice());
    assert_eq!(event._amount, withdrawal_amount);
    assert_eq!(
        event._additionalData,
        alloy::primitives::Bytes::from(additional_data)
    );
}

#[test]
fn test_l2_base_token_withdraw_with_dirty_address() {
    let mut chain = Chain::empty(None);

    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(1000000000000000000u64); // 1 ETH

    // Deliberately set invalid balance (insufficient funds)
    // Set up initial balance for the sender
    chain.set_balance(B160::from_be_bytes(sender.into_array()), withdrawal_amount);

    // Prepare withdraw(address) calldata
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("51cff8d9").unwrap()); // withdraw selector
    calldata.extend_from_slice(&[1u8; 12]); // "dirty" padding for address
    calldata.extend_from_slice(l1_receiver.as_slice()); // l1_receiver address

    let tx = TransactionRequest {
        chain_id: Some(37),
        from: Some(sender),
        to: Some(TxKind::Call(l2_base_token_address)),
        input: calldata.into(),
        value: Some(withdrawal_amount),
        gas: Some(200_000),
        max_fee_per_gas: Some(1000),
        max_priority_fee_per_gas: Some(1000),
        nonce: Some(0),
        ..TransactionRequest::default()
    };

    let encoded_tx = rig::utils::encode_l1_tx(tx);
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None);

    // Assert transaction failed due to insufficient balance
    assert!(
        output.tx_results.iter().any(|r| {
            if let Ok(tx_result) = r {
                !tx_result.is_success()
            } else {
                true // Transaction errors also count as failures
            }
        }),
        "Transaction should fail with incorrect calldata"
    );
}

#[test]
fn test_l2_base_token_withdraw_with_message_with_dirty_address() {
    let mut chain = Chain::empty(None);

    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH
    let additional_data = b"test message data";

    // Set up initial balance for the sender
    chain.set_balance(B160::from_be_bytes(sender.into_array()), withdrawal_amount);

    // Prepare withdrawWithMessage(address,bytes) calldata
    // withdrawWithMessage(address,bytes) has selector 0x84bc3eb0
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("84bc3eb0").unwrap()); // withdrawWithMessage selector
    calldata.extend_from_slice(&[1u8; 12]); // "dirty" padding for address
    calldata.extend_from_slice(l1_receiver.as_slice()); // l1_receiver address

    // Offset to the bytes data (0x40 = 64)
    calldata.extend_from_slice(&[0u8; 31]);
    calldata.push(0x40);

    // Length of additional data
    calldata.extend_from_slice(&[0u8; 31]);
    calldata.push(additional_data.len() as u8);

    // Additional data, padded to 32 bytes
    calldata.extend_from_slice(additional_data);
    let padding_needed = 32 - (additional_data.len() % 32);
    if padding_needed != 32 {
        calldata.extend_from_slice(&vec![0u8; padding_needed]);
    }

    let tx = TransactionRequest {
        chain_id: Some(37),
        from: Some(sender),
        to: Some(TxKind::Call(l2_base_token_address)),
        input: calldata.into(),
        value: Some(withdrawal_amount),
        gas: Some(300_000),
        max_fee_per_gas: Some(1000),
        max_priority_fee_per_gas: Some(1000),
        nonce: Some(0),
        ..TransactionRequest::default()
    };

    let encoded_tx = rig::utils::encode_l1_tx(tx);
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None);

    // Assert transaction failed due to insufficient balance
    assert!(
        output.tx_results.iter().any(|r| {
            if let Ok(tx_result) = r {
                !tx_result.is_success()
            } else {
                true // Transaction errors also count as failures
            }
        }),
        "Transaction should fail with incorrect calldata"
    );
}
