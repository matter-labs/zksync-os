//!
//! These tests are focused on system hooks functionality.
//!
#![cfg(test)]

use alloy_sol_types::{sol, SolEvent};
use rig::alloy::primitives::address;
use rig::alloy::primitives::Address;
use rig::ruint::aliases::B160;
use rig::ruint::aliases::U256;
use rig::system_hooks::addresses_constants::L2_INTEROP_ROOT_STORAGE_ADDRESS;
use rig::system_hooks::addresses_constants::SYSTEM_CONTEXT_ADDRESS;
use rig::testing_utils::call_address_and_measure_gas_cost;
use rig::tx_failed;
use rig::tx_succeeded;
use rig::utils::{
    address_into_special_storage_key, AccountProperties, L1TxBuilder,
    ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use rig::zk_ee::utils::Bytes32;
use rig::zksync_os_interface::types::{ExecutionOutput, ExecutionResult};
use rig::{alloy, TestingFramework};

#[test]
fn test_value_transfer_fails_if_insufficient_balance_max_msg_value() {
    let sender = address!("1234567890123456789012345678901234567890");
    let recipient = address!("2222567890123456789012345678901234567890");

    // Sender has 1 wei, tries to send 2^256 wei.
    let initial_sender = alloy::primitives::U256::from(1u64);
    let value = alloy::primitives::U256::MAX;

    let mut tester = TestingFramework::new().with_balance(sender, initial_sender);

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(recipient)
        .input(Vec::new())
        .value(value)
        // keep fees at 0 so we can assert balances are unchanged on failure
        .gas_price(0)
        .gas_limit(200_000)
        .nonce(0)
        .build();
    let output = tester.execute_block(vec![tx]);

    assert!(
        tx_failed(&output, 0),
        "tx must fail when msg.value > sender balance"
    );

    // Balances must be unchanged (no fees).
    assert_eq!(
        tester.get_balance(&sender),
        initial_sender,
        "sender balance must not change"
    );
    assert_eq!(
        tester.get_balance(&recipient),
        alloy::primitives::U256::ZERO,
        "recipient must not receive value"
    );
}

#[test]
fn test_l2_base_token_withdraw_fails_if_insufficient_balance() {
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");

    // Sender has 1 wei, tries to withdraw 2 eth.
    let initial_sender = alloy::primitives::U256::from(1u64);
    let value = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH

    let mut tester = TestingFramework::new().with_balance(sender, initial_sender);

    // withdraw(address) selector 0x51cff8d9
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("51cff8d9").unwrap());
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(l1_receiver.as_slice());

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(l2_base_token_address)
        .input(calldata)
        .value(value)
        .gas_price(0)
        .gas_limit(200_000)
        .nonce(0)
        .build();
    let output = tester.execute_block(vec![tx]);

    assert!(
        tx_failed(&output, 0),
        "withdraw must fail when msg.value > sender balance"
    );

    // Balances unchanged
    assert_eq!(
        tester.get_balance(&sender),
        initial_sender,
        "sender balance must not change"
    );

    // No Withdrawal event must be emitted.
    sol! {
        event Withdrawal(address indexed _l2Sender, address indexed _l1Receiver, uint256 _amount);
    }
    let any_withdrawal = output.tx_results[0]
        .as_ref()
        .ok()
        .map(|r| {
            r.logs.iter().any(|ev| {
                ev.address == l2_base_token_address && Withdrawal::decode_log_data(ev).is_ok()
            })
        })
        .unwrap_or(false);

    assert!(
        !any_withdrawal,
        "Withdrawal event must not be emitted on insufficient funds"
    );
}

#[test]
fn test_l2_base_token_withdraw_with_message_fails_if_insufficient_balance() {
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let additional_data = b"test message data";

    // Sender has 1 wei, tries to withdrawWithMessage 2 eth.
    let initial_sender = alloy::primitives::U256::from(1u64);
    let value = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH

    let mut tester = TestingFramework::new().with_balance(sender, initial_sender);

    // withdrawWithMessage(address,bytes) selector 0x84bc3eb0
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("84bc3eb0").unwrap());
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(l1_receiver.as_slice());

    // offset to bytes data (0x40)
    calldata.extend_from_slice(&[0u8; 31]);
    calldata.push(0x40);

    // length
    calldata.extend_from_slice(&[0u8; 31]);
    calldata.push(additional_data.len() as u8);

    // bytes data padded
    calldata.extend_from_slice(additional_data);
    let padding_needed = 32 - (additional_data.len() % 32);
    if padding_needed != 32 {
        calldata.extend_from_slice(&vec![0u8; padding_needed]);
    }

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(l2_base_token_address)
        .input(calldata)
        .value(value)
        .gas_price(0)
        .gas_limit(300_000)
        .nonce(0)
        .build();
    let output = tester.execute_block(vec![tx]);

    assert!(
        tx_failed(&output, 0),
        "withdrawWithMessage must fail when msg.value > sender balance"
    );

    // Balances unchanged
    assert_eq!(
        tester.get_balance(&sender),
        initial_sender,
        "sender balance must not change"
    );

    // No WithdrawalWithMessage event must be emitted.
    sol! {
        event WithdrawalWithMessage(address indexed _l2Sender, address indexed _l1Receiver, uint256 _amount, bytes _additionalData);
    }
    let any_event = output.tx_results[0]
        .as_ref()
        .ok()
        .map(|r| {
            r.logs.iter().any(|ev| {
                ev.address == l2_base_token_address
                    && WithdrawalWithMessage::decode_log_data(ev).is_ok()
            })
        })
        .unwrap_or(false);

    assert!(
        !any_event,
        "WithdrawalWithMessage event must not be emitted on insufficient funds"
    );
}

/// With sufficient existing L2 balance, an L1 tx with non-zero `value` should succeed and
/// transfer funds to the recipient (spending from sender’s L2 balance).
#[test]
fn test_l1_value_transfer_spends_from_l2_balance() {
    let sender = address!("1234567890123456789012345678901234567890");
    let recipient = address!("2222567890123456789012345678901234567890");
    let value = alloy::primitives::U256::from(1_000_000_000_000_000_000u64); // 1 ETH

    // Fund sender so `msg.value` can be paid from L2 balance.
    let mut tester = TestingFramework::new().with_balance(sender, value);

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(recipient)
        .input(Vec::new())
        .value(value)
        // keep fees minimal to reduce side-effects
        .gas_price(0)
        .gas_limit(200_000)
        .nonce(0)
        .build();
    let output = tester.execute_block(vec![tx]);

    assert!(
        tx_succeeded(&output, 0),
        "tx must succeed with sufficient L2 balance"
    );
    assert_eq!(
        tester.get_balance(&recipient),
        value,
        "recipient must receive msg.value"
    );
}

#[test]
fn test_set_bytecode_details_evm() {
    let contract_deployer_address = address!("0000000000000000000000000000000000008006");
    let contract_deployer_hook_address = address!("0000000000000000000000000000000000007002");

    let bytecode = hex::decode("0123456789").unwrap();
    let code_hash = Bytes32::from_array(
        hex::decode("1c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let calldata =
        hex::decode("00000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b944")
            .unwrap();

    let mut tester = TestingFramework::new()
        .with_preimage(code_hash, &bytecode)
        .with_balance(
            contract_deployer_address,
            U256::from(1_000_000_000_000_000_u64),
        );

    let tx = L1TxBuilder::new()
        .from(contract_deployer_address)
        .to(contract_deployer_hook_address)
        .input(calldata)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

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
fn test_contract_deployer_temp_hook() {
    let complex_upgrader_address = address!("000000000000000000000000000000000000800f");
    let contract_deployer_temp_hook_address = address!("0000000000000000000000000000000000008006");

    let bytecode = hex::decode("0123456789").unwrap();
    let code_hash = Bytes32::from_array(
        hex::decode("1c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    // setBytecodeDetailsEVM(address,bytes32,uint32,bytes32)
    let calldata =
        hex::decode("f6eca0b000000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b944")
            .unwrap();

    let mut tester = TestingFramework::new()
        .with_preimage(code_hash, &bytecode)
        .with_balance(
            complex_upgrader_address,
            U256::from(1_000_000_000_000_000_u64),
        );

    let tx = L1TxBuilder::new()
        .from(complex_upgrader_address)
        .to(contract_deployer_temp_hook_address)
        .input(calldata)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

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
fn test_set_bytecode_on_address_unauthorized_pretends_empty_and_no_gas_burn() {
    let unauthorized_from = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let set_bytecode_hook_address = address!("0000000000000000000000000000000000007002");

    let calldata =
        hex::decode("00000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b944")
            .unwrap();

    let tx = L1TxBuilder::new()
        .from(unauthorized_from)
        .to(set_bytecode_hook_address)
        .input(calldata.clone())
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let mut tester = TestingFramework::new();
    let output = tester.execute_block(vec![tx]);

    let tx_result = &output
        .tx_results
        .first()
        .unwrap()
        .as_ref()
        .unwrap()
        .execution_result;
    match tx_result {
        ExecutionResult::Success(ExecutionOutput::Call(return_data)) => {
            assert!(
                return_data.is_empty(),
                "unauthorized call must return empty data"
            );
        }
        _ => panic!("unauthorized call must succeed as empty account, got: {tx_result:?}"),
    }

    // The call must not perform code deployment writes.
    let deployment_write = output.storage_writes.iter().find(|write| {
        write.account.0 == ACCOUNT_PROPERTIES_STORAGE_ADDRESS.to_be_bytes()
            && write.account_key.0
                == address_into_special_storage_key(&B160::from_limbs([0x10002, 0, 0]))
                    .as_u8_array()
    });
    assert!(
        deployment_write.is_none(),
        "unauthorized caller must not write bytecode details"
    );

    let gas_used = call_address_and_measure_gas_cost(
        set_bytecode_hook_address,
        unauthorized_from,
        0,
        calldata,
        vec![],
    );
    assert_eq!(gas_used, 0, "hook must not burn EVM gas");
}

#[test]
fn test_contract_deployer_temp_hook_unauthorized_pretends_empty_and_no_gas_burn() {
    let unauthorized_from = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let contract_deployer_temp_hook_address = address!("0000000000000000000000000000000000008006");

    let calldata =
        hex::decode("f6eca0b000000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b944")
            .unwrap();

    let tx = L1TxBuilder::new()
        .from(unauthorized_from)
        .to(contract_deployer_temp_hook_address)
        .input(calldata.clone())
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let mut tester = TestingFramework::new();
    let output = tester.execute_block(vec![tx]);

    let tx_result = &output
        .tx_results
        .first()
        .unwrap()
        .as_ref()
        .unwrap()
        .execution_result;
    match tx_result {
        ExecutionResult::Success(ExecutionOutput::Call(return_data)) => {
            assert!(
                return_data.is_empty(),
                "unauthorized call must return empty data"
            );
        }
        _ => panic!("unauthorized call must succeed as empty account, got: {tx_result:?}"),
    }

    // The call must not perform code deployment writes.
    let deployment_write = output.storage_writes.iter().find(|write| {
        write.account.0 == ACCOUNT_PROPERTIES_STORAGE_ADDRESS.to_be_bytes()
            && write.account_key.0
                == address_into_special_storage_key(&B160::from_limbs([0x10002, 0, 0]))
                    .as_u8_array()
    });
    assert!(
        deployment_write.is_none(),
        "unauthorized caller must not write bytecode details"
    );

    let gas_used = call_address_and_measure_gas_cost(
        contract_deployer_temp_hook_address,
        unauthorized_from,
        0,
        calldata,
        vec![],
    );
    assert_eq!(gas_used, 0, "hook must not burn EVM gas");
}

#[test]
fn test_l1_messenger_hook_succeeds() {
    // making sure hooks are installed
    let mut tester = TestingFramework::new().with_system_contracts(false, false);

    let l1_messenger_contract = address!("0000000000000000000000000000000000008008");

    let l1_messenger_hook = address!("0000000000000000000000000000000000007001");

    // Calldata that the hook *expects*:
    // abi.encode(msg.sender, _message)
    let hook_calldata = hex::decode(
        "000000000000000000000000111111111111111111111111111111111111111100000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000020000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    .unwrap();

    let tx = L1TxBuilder::new()
        .from(l1_messenger_contract)
        .to(l1_messenger_hook)
        .input(hook_calldata)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

    let tx_result = &output
        .tx_results
        .first()
        .unwrap()
        .as_ref()
        .unwrap()
        .execution_result;

    match tx_result {
        ExecutionResult::Success(_) => {
            // ok
        }
        _ => {
            panic!("L1 messenger hook call from authorized sender did not succeed: {tx_result:?}");
        }
    }
}

#[test]
fn test_l1_messenger_hook_fails_with_invalid_calldata() {
    // making sure hooks are installed
    let mut tester = TestingFramework::new().with_system_contracts(false, false);

    let l1_messenger_contract = address!("0000000000000000000000000000000000008008");

    let l1_messenger_hook = address!("0000000000000000000000000000000000007001");

    // Invalid calldata
    let hook_calldata = hex::decode("00000000000000000000000011111111").unwrap();

    let tx = L1TxBuilder::new()
        .from(l1_messenger_contract)
        .to(l1_messenger_hook)
        .input(hook_calldata)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

    let tx_result = &output
        .tx_results
        .first()
        .unwrap()
        .as_ref()
        .unwrap()
        .execution_result;

    assert!(matches!(tx_result, ExecutionResult::Revert { .. }));
}

#[test]
fn test_l1_messenger_hook_unauthorized_sender_ignored() {
    // making sure hooks are installed
    let mut tester = TestingFramework::new().with_system_contracts(false, false);

    // ❌ this should NOT be the L1Messenger system contract address
    let unauthorized_from = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

    let l1_messenger_hook = address!("0000000000000000000000000000000000007001");

    // Calldata that the hook *expects*:
    // abi.encode(msg.sender, _message)
    // For the unauthorized test we don't care about the message contents,
    // we just want msg.sender (on the hook side) to be wrong (EOA instead of system contract).
    let hook_calldata = hex::decode(
    "000000000000000000000000111111111111111111111111111111111111111100000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000020000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    )
    .unwrap();

    let tx = L1TxBuilder::new()
        .from(unauthorized_from)
        .to(l1_messenger_hook)
        .input(hook_calldata.clone())
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

    let tx_result = &output
        .tx_results
        .first()
        .unwrap()
        .as_ref()
        .unwrap()
        .execution_result;

    match tx_result {
        ExecutionResult::Success(ExecutionOutput::Call(return_data)) => {
            assert!(
                return_data.is_empty(),
                "unauthorized call must return empty data"
            );
        }
        _ => panic!("unauthorized call must succeed as empty account, got: {tx_result:?}"),
    }

    let logs = &output.tx_results[0].as_ref().unwrap().logs;
    assert!(logs.is_empty(), "unauthorized caller must not emit logs");

    let gas_used = call_address_and_measure_gas_cost(
        l1_messenger_hook,
        unauthorized_from,
        0,
        hook_calldata,
        vec![],
    );
    assert_eq!(gas_used, 0, "hook must not burn EVM gas");
}

#[test]
fn test_l2_base_token_withdraw_events() {
    // L2 base token address is 0x800a
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(1000000000000000000u64); // 1 ETH

    let mut tester = TestingFramework::new()
        .with_system_contracts(true, true)
        .with_balance(sender, withdrawal_amount);

    // Prepare withdraw(address) calldata
    // withdraw(address) has selector 0x51cff8d9
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("51cff8d9").unwrap()); // withdraw selector
    calldata.extend_from_slice(&[0u8; 12]); // padding for address
    calldata.extend_from_slice(l1_receiver.as_slice()); // l1_receiver address

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(l2_base_token_address)
        .input(calldata)
        .value(withdrawal_amount)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

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
        .find(|event| {
            event.address == l2_base_token_address && Withdrawal::decode_log_data(&event).is_ok()
        });
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
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH
    let additional_data = b"test message data";

    // Set up initial balance for the sender
    let mut tester = TestingFramework::new()
        .with_system_contracts(true, true)
        .with_balance(sender, withdrawal_amount);

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

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(l2_base_token_address)
        .input(calldata)
        .value(withdrawal_amount)
        .gas_price(1000)
        .gas_limit(300_000)
        .build();

    let output = tester.execute_block(vec![tx]);

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
        .find(|event| {
            event.address == l2_base_token_address
                && WithdrawalWithMessage::decode_log_data(&event).is_ok()
        });
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
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(1000000000000000000u64); // 1 ETH

    // Deliberately set invalid balance (insufficient funds)
    // Set up initial balance for the sender
    let mut tester = TestingFramework::new()
        .with_system_contracts(true, true)
        .with_balance(sender, withdrawal_amount);

    // Prepare withdraw(address) calldata
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("51cff8d9").unwrap()); // withdraw selector
    calldata.extend_from_slice(&[1u8; 12]); // "dirty" padding for address
    calldata.extend_from_slice(l1_receiver.as_slice()); // l1_receiver address

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(l2_base_token_address)
        .input(calldata)
        .value(withdrawal_amount)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

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
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH
    let additional_data = b"test message data";

    // Set up initial balance for the sender
    let mut tester = TestingFramework::new()
        .with_system_contracts(true, true)
        .with_balance(sender, withdrawal_amount);

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

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(l2_base_token_address)
        .input(calldata)
        .value(withdrawal_amount)
        .gas_price(1000)
        .gas_limit(300_000)
        .build();

    let output = tester.execute_block(vec![tx]);

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
fn test_l2_base_token_no_mint_event_regression() {
    // L2 base token address is 0x800a
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let recipient = address!("2222567890123456789012345678901234567890");
    let mint_amount = alloy::primitives::U256::from(5000000000000000000u64); // 5 ETH

    let mut tester = TestingFramework::new().with_balance(sender, mint_amount);

    // Prepare mint calldata - typically this would be called by the bootloader or bridge
    // For testing purposes, we'll simulate a mint by sending ETH value to the base token contract
    // The mint event should be emitted when the contract receives value

    // Create a transaction that sends ETH to the L2 base token contract
    // This simulates a bridge deposit or native token mint
    let tx = L1TxBuilder::new()
        .from(sender)
        .to(recipient)
        .value(mint_amount)
        .gas_price(1000)
        .gas_limit(100_000)
        .build();

    let output = tester.execute_block(vec![tx]);

    // Assert transaction succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {} failed with: {:?}", i, r)
        }
        success
    }));

    sol! {
        event Mint(address indexed _account, uint256 _amount);
    }

    // Check if mint event was not emitted
    let mint_events: Vec<_> = output.tx_results[0]
        .as_ref()
        .unwrap()
        .logs
        .iter()
        .filter(|event| {
            event.address == l2_base_token_address && Mint::decode_log_data(&event).is_ok()
        })
        .collect();

    assert!(mint_events.is_empty(), "Mint event should not be emitted");
}

#[test]
fn test_contract_deployer_gas_charging() {
    let contract_deployer_address = address!("0000000000000000000000000000000000008006");
    let contract_deployer_hook_address = address!("0000000000000000000000000000000000007002");

    let bytecode = hex::decode("0123456789").unwrap();
    let code_hash = Bytes32::from_array(
        hex::decode("1c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let calldata =
        hex::decode("00000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b944")
            .unwrap();

    let gas_used = call_address_and_measure_gas_cost(
        contract_deployer_hook_address,
        contract_deployer_address,
        0,
        calldata,
        vec![(code_hash, bytecode)],
    );

    // The hook should charge for bytecode length
    assert_eq!(gas_used, 2850);
}

#[test]
fn test_l1_messenger_gas_charging() {
    let l1_messenger_address = address!("0000000000000000000000000000000000008008");
    let sender = address!("1234567890123456789012345678901234567890");

    // sendToL1(bytes) - 62f84b24
    let message = b"test message to L1";
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("62f84b24").unwrap()); // sendToL1 selector
    calldata.extend_from_slice(&[0u8; 31]); // offset padding
    calldata.push(0x20); // offset to data (32 bytes)
    calldata.extend_from_slice(&[0u8; 31]); // length padding
    calldata.push(message.len() as u8); // message length
    calldata.extend_from_slice(message); // message data
                                         // Pad to 32 byte boundary
    let padding_needed = 32 - (message.len() % 32);
    if padding_needed != 32 {
        calldata.extend_from_slice(&vec![0u8; padding_needed]);
    }

    let gas_used =
        call_address_and_measure_gas_cost(l1_messenger_address, sender, 0, calldata, vec![]);

    // Verify that gas was charged - this should include the hook gas cost + keccak + LOG costs
    // The hook should charge keccak256 costs + LOG costs
    assert_eq!(gas_used, 9238);
}

#[test]
fn test_l2_base_token_withdraw_gas_charging() {
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");

    // Prepare withdraw(address) calldata - 51cff8d9
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("51cff8d9").unwrap()); // withdraw selector
    calldata.extend_from_slice(&[0u8; 12]); // padding for address
    calldata.extend_from_slice(l1_receiver.as_slice()); // l1_receiver address

    let gas_used = call_address_and_measure_gas_cost(
        l2_base_token_address,
        sender,
        1000000000000000000u64,
        calldata,
        vec![],
    );

    assert_eq!(gas_used, 52401);
}

#[test]
fn test_l2_base_token_withdraw_with_message_gas_charging() {
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let additional_data = b"test message data";

    // Prepare withdrawWithMessage(address,bytes) calldata - 84bc3eb0
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

    let gas_used = call_address_and_measure_gas_cost(
        l2_base_token_address,
        sender,
        2000000000000000000u64,
        calldata,
        vec![],
    );

    // Verify that gas was charged - this should include hook gas cost + memory copy costs + L1 message costs + event costs
    // The hook should charge copy costs + L1 message costs + event emission costs
    assert_eq!(gas_used, 54440);
}

#[test]
fn test_mint_base_token_hook() {
    let mut tester = TestingFramework::new().with_minted_tokens_to_treasury();

    // L2 base token address is the only address allowed to call the mint hook
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    // Mint hook address (0x7100)
    let mint_hook_address = address!("0000000000000000000000000000000000007100");
    let mint_amount = alloy::primitives::U256::from(3000000000000000000u64); // 3 ETH

    // Check initial balance of L2_BASE_TOKEN_ADDRESS is zero
    let initial_balance = tester
        .get_account_properties(&l2_base_token_address)
        .balance;

    // Prepare calldata: 32 bytes containing the mint amount as U256 big-endian
    let calldata = mint_amount.to_be_bytes::<32>().to_vec();

    // Create transaction from L2_BASE_TOKEN_ADDRESS to MINT_HOOK_ADDRESS
    let tx = L1TxBuilder::new()
        .from(l2_base_token_address)
        .to(mint_hook_address)
        .input(calldata)
        .value(alloy::primitives::U256::ZERO) // No ETH value needed for mint
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

    // Assert transaction succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {} failed with: {:?}", i, r)
        }
        success
    }));

    // Check that the caller's (L2_BASE_TOKEN_ADDRESS) balance was increased by the mint amount
    let final_balance = tester
        .get_account_properties(&l2_base_token_address)
        .balance;

    let actually_minted_amount = final_balance
        .checked_sub(initial_balance)
        .expect("Some tokens should be minted");
    assert_eq!(
        actually_minted_amount, mint_amount,
        "Minted amount should match the requested mint amount"
    );
}

#[test]
fn test_mint_base_token_hook_rejects_non_zero_value() {
    let mut tester = TestingFramework::new().with_minted_tokens_to_treasury();

    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let mint_hook_address = address!("0000000000000000000000000000000000007100");
    let mint_amount = alloy::primitives::U256::from(3000000000000000000u64);
    let call_value = alloy::primitives::U256::from(1u64);

    tester.set_balance(l2_base_token_address, call_value);

    let initial_balance = tester
        .get_account_properties(&l2_base_token_address)
        .balance;

    let calldata = mint_amount.to_be_bytes::<32>().to_vec();

    let tx = L1TxBuilder::new()
        .from(l2_base_token_address)
        .to(mint_hook_address)
        .input(calldata)
        .value(call_value)
        .gas_price(0)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);
    let tx_result = output.tx_results[0]
        .as_ref()
        .expect("Mint hook call should be processed");
    assert!(
        !tx_result.is_success(),
        "Mint hook should fail when called with non-zero value"
    );

    let final_balance = tester
        .get_account_properties(&l2_base_token_address)
        .balance;

    let balance_delta = final_balance
        .checked_sub(initial_balance)
        .expect("Final balance should not be below initial balance");
    assert!(
        balance_delta <= call_value,
        "Mint amount should not be credited when value is non-zero"
    );
}

#[test]
fn test_event_hooks_empty_topics() {
    for test_contract_address in [L2_INTEROP_ROOT_STORAGE_ADDRESS, SYSTEM_CONTEXT_ADDRESS] {
        // Contract that emits a log with empty topics array - this should be handled gracefully
        let test_contract = Address::from(test_contract_address.to_be_bytes());

        // Bytecode that emits LOG0 (no topics)
        // PUSH1 0x00    -> 6000  (data offset)
        // PUSH1 0x00    -> 6000  (data length)
        // LOG0          -> a0    (emit log with no topics)
        // STOP          -> 00
        let test_contract_bytecode = hex::decode("60006000a000").unwrap();
        let mut tester =
            TestingFramework::new().with_evm_contract(test_contract, &test_contract_bytecode);

        let tx = L1TxBuilder::new()
            .from(address!("1234567890123456789012345678901234567890"))
            .to(test_contract)
            .input(hex::decode("").unwrap())
            .gas_price(1000)
            .gas_limit(200_000)
            .build();

        let output = tester.execute_block(vec![tx]);

        // Transaction should succeed - empty topics should be handled gracefully
        assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
            let success = r.clone().is_ok_and(|o| o.is_success());
            if !success {
                panic!("({}) Transaction {} failed with: {:?}", test_contract, i, r)
            }
            success
        }));
    }
}

// ---------------------------------------------------------------------------
// FRI precompile tests
// ---------------------------------------------------------------------------
//
// The full verification path (real Airbender proof → precompile returns true)
// requires a proof fixture and is not covered here. These tests exercise:
//   1. The precompile is not registered on non-gateway chains.
//   2. On a gateway chain, querying an unverified statement hash returns false.
//   3. FRI_PROOF_TX_TYPE is rejected when is_gateway = false.
mod fri_precompile {
    use super::*;
    use rig::alloy::eips::eip2930::AccessList;
    use rig::alloy::primitives::B256;
    use rig::zksync_os_interface::types::ExecutionOutput;
    use zksync_os_tests_common::zksync_tx::{
        fri_proof_tx::UnsignedZKsyncFriProofTx, ZKsyncTxEnvelope,
    };

    // The FRI precompile lives at address 0x0000...0101 (FRI_PRECOMPILE_ADDRESS_LOW = 0x0101).
    const FRI_PRECOMPILE: Address = address!("0000000000000000000000000000000000000101");

    /// Calling the FRI precompile on a non-gateway chain: the address has no
    /// code and the call returns empty data (behaves like a call to an EOA),
    /// not an error.  The precompile is not registered so the call is a no-op.
    #[test]
    fn fri_precompile_not_registered_on_non_gateway_chain() {
        let mut tester = TestingFramework::new();
        let wallet = tester.prefunded_random_signer();

        // 32-byte input: some statement hash
        let input = [0xabu8; 32];

        let tx = ZKsyncTxEnvelope::from_eth_tx(
            rig::alloy::consensus::TxLegacy {
                chain_id: Some(37),
                nonce: 0,
                gas_price: 25_000,
                gas_limit: 200_000,
                to: rig::alloy::primitives::TxKind::Call(FRI_PRECOMPILE),
                value: Default::default(),
                input: input.to_vec().into(),
            },
            wallet,
        );

        let output = tester.execute_block(vec![tx]);

        // Tx succeeds (calls empty account), but returns no data — precompile not installed.
        assert!(
            tx_succeeded(&output, 0),
            "call to unregistered address must succeed"
        );
        let result = output.tx_results[0].as_ref().unwrap();
        match &result.execution_result {
            rig::zksync_os_interface::types::ExecutionResult::Success(ExecutionOutput::Call(
                data,
            )) => {
                assert!(data.is_empty(), "unregistered address must return no data");
            }
            other => panic!("expected success with empty return, got: {other:?}"),
        }
    }

    /// On a gateway chain, querying the FRI precompile with a hash that was
    /// never verified returns ABI-encoded false (32 zero bytes).
    #[test]
    fn fri_precompile_returns_false_for_unverified_hash_on_gateway() {
        let mut tester = TestingFramework::new().with_gateway_mode();
        let wallet = tester.prefunded_random_signer();

        // Any 32-byte statement hash that was never submitted as a proof.
        let unverified_hash = [0xddu8; 32];

        let tx = ZKsyncTxEnvelope::from_eth_tx(
            rig::alloy::consensus::TxLegacy {
                chain_id: Some(37),
                nonce: 0,
                gas_price: 25_000,
                gas_limit: 200_000,
                to: rig::alloy::primitives::TxKind::Call(FRI_PRECOMPILE),
                value: Default::default(),
                input: unverified_hash.to_vec().into(),
            },
            wallet,
        );

        let output = tester.execute_block(vec![tx]);

        assert!(
            tx_succeeded(&output, 0),
            "call to FRI precompile must succeed"
        );
        let result = output.tx_results[0].as_ref().unwrap();
        match &result.execution_result {
            rig::zksync_os_interface::types::ExecutionResult::Success(ExecutionOutput::Call(
                data,
            )) => {
                assert_eq!(data.len(), 32, "precompile must return 32 bytes");
                assert_eq!(
                    data.as_slice(),
                    &[0u8; 32],
                    "unverified hash must return false (all zeros)"
                );
            }
            other => panic!("expected success with false return, got: {other:?}"),
        }
    }

    /// A FRI_PROOF_TX_TYPE transaction submitted to a non-gateway chain must
    /// be rejected during validation — it must not be included in the block.
    #[test]
    fn fri_proof_tx_rejected_on_non_gateway_chain() {
        let mut tester = TestingFramework::new();
        let wallet = tester.prefunded_random_signer();

        // Construct a minimal FRI proof tx with one dummy statement hash.
        // The sidecar oracle is empty (NoFriProofSidecar), but the tx should be
        // rejected before sidecar resolution because is_gateway = false.
        let statement_hash = B256::from([0x42u8; 32]);
        let unsigned = UnsignedZKsyncFriProofTx {
            chain_id: 37,
            nonce: 0,
            max_priority_fee_per_gas: 1_000,
            max_fee_per_gas: 25_000,
            gas_limit: 500_000,
            to: FRI_PRECOMPILE,
            value: Default::default(),
            input: Default::default(),
            access_list: AccessList::default(),
            statement_versioned_hashes: vec![statement_hash],
        };
        let signed = unsigned.sign(wallet);
        let tx = ZKsyncTxEnvelope::FriProof(signed);

        let output = tester.execute_block(vec![tx]);

        // The tx must be rejected (not just reverted) — tx_results entry is Err.
        assert!(
            output.tx_results[0].is_err(),
            "FRI_PROOF_TX on non-gateway chain must be rejected, got: {:?}",
            output.tx_results[0]
        );
    }
}

// ---------------------------------------------------------------------------
// Real-proof end-to-end FRI precompile test
//
// Gated behind the `fri_proof_e2e` Cargo feature so the normal workspace build
// stays fast. Run with:
//
//   AIRBENDER_DEV_PATH=/Users/ra/Work/ZkSync/zksync-airbender \
//   cargo test -p system_hooks_tests \
//     --features system_hooks_tests/fri_proof_e2e \
//     fri_precompile_e2e
//
// Override defaults with FRI_ORACLE_PATH, FRI_SETUP_PATH, and FRI_LAYOUT_PATH.
// ---------------------------------------------------------------------------
#[cfg(feature = "fri_proof_e2e")]
mod fri_precompile_e2e {
    use super::*;
    use rig::alloy::eips::eip2930::AccessList;
    use rig::alloy::primitives::B256;
    use rig::zksync_os_interface::types::ExecutionOutput;
    use zksync_os_tests_common::zksync_tx::{
        fri_proof_tx::UnsignedZKsyncFriProofTx, ZKsyncTxEnvelope,
    };

    use execution_utils::setups::{read_and_pad_binary, CompiledCircuitsSet};
    use execution_utils::unified_circuit::{
        flatten_proof_into_responses_for_unified_recursion,
        get_unified_circuit_artifact_for_machine_type, verify_proof_in_unified_layer,
    };
    use execution_utils::unrolled::{UnrolledProgramProof, UnrolledProgramSetup};
    use flate2::read::GzDecoder;
    use riscv_transpiler::cycle::IWithoutByteAccessIsaConfigWithDelegation;
    use std::io::Read;
    use std::path::PathBuf;

    const FRI_STATEMENT_HASH_VERSION: u8 = 1;
    // Address 0x0101 — the FRI precompile.
    const FRI_PRECOMPILE: Address = address!("0000000000000000000000000000000000000101");

    fn repo_root() -> PathBuf {
        // CARGO_MANIFEST_DIR = tests/instances/system_hooks
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repo root is three directories above the crate manifest")
            .to_path_buf()
    }

    fn resolve_path(env_var: &str, default_relative: &str) -> PathBuf {
        if let Ok(v) = std::env::var(env_var) {
            PathBuf::from(v)
        } else {
            repo_root().join(default_relative)
        }
    }

    fn maybe_decompress(bytes: &[u8]) -> Vec<u8> {
        if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut out = Vec::new();
            GzDecoder::new(bytes)
                .read_to_end(&mut out)
                .expect("gzip decompress");
            out
        } else {
            bytes.to_vec()
        }
    }

    fn decode_bincode<T: serde::de::DeserializeOwned>(bytes: &[u8], label: &str) -> T {
        let (val, _): (T, usize) =
            bincode::serde::decode_from_slice(bytes, bincode::config::standard())
                .unwrap_or_else(|e| panic!("failed to decode {label}: {e}"));
        val
    }

    fn encode_bincode<T: serde::Serialize>(value: &T, label: &str) -> Vec<u8> {
        bincode::serde::encode_to_vec(value, bincode::config::standard())
            .unwrap_or_else(|e| panic!("failed to encode {label}: {e}"))
    }

    fn airbender_dev_path() -> PathBuf {
        std::env::var("AIRBENDER_DEV_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/Users/ra/Work/ZkSync/zksync-airbender"))
    }

    fn setup_from_proof(proof: &UnrolledProgramProof) -> UnrolledProgramSetup {
        let (&family_idx, family_proofs) = proof
            .circuit_families_proofs
            .iter()
            .next()
            .expect("proof must contain a unified circuit family proof");
        assert_eq!(
            proof.circuit_families_proofs.len(),
            1,
            "unified recursion proof must contain exactly one circuit family"
        );
        let first_proof = family_proofs
            .first()
            .expect("unified circuit family must contain at least one proof");
        assert_eq!(
            first_proof.setup_tree_caps.len(),
            2,
            "expected exactly two setup caps for the unified verifier"
        );

        let setup_caps = core::array::from_fn(|idx| {
            first_proof.setup_tree_caps[idx]
                .clone()
                .into_fixed_holder::<64>()
        });

        UnrolledProgramSetup {
            expected_final_pc: proof.final_pc,
            binary_hash: [0u8; 32],
            circuit_families_setups: std::collections::BTreeMap::from([(family_idx, setup_caps)]),
            // Not consumed by `flatten_unified_for_recursion`, but the struct
            // is shared with the unrolled verifier path and requires the field.
            inits_and_teardowns_setup: setup_caps,
            end_params: [0u32; 8],
        }
    }

    fn ensure_recursion_unified_artifacts(
        proof: &UnrolledProgramProof,
        setup_path: &PathBuf,
        layout_path: &PathBuf,
    ) -> (UnrolledProgramSetup, CompiledCircuitsSet) {
        let setup = if setup_path.exists() {
            let setup_bytes = std::fs::read(setup_path).expect("read setup");
            decode_bincode(&setup_bytes, "UnrolledProgramSetup")
        } else {
            let setup = setup_from_proof(proof);
            if let Some(parent) = setup_path.parent() {
                std::fs::create_dir_all(parent).expect("create FRI fixture directory");
            }
            std::fs::write(
                setup_path,
                encode_bincode(&setup, "UnrolledProgramSetup fixture"),
            )
            .expect("write generated FRI setup fixture");
            setup
        };

        if layout_path.exists() {
            let layout_bytes = std::fs::read(layout_path).expect("read layout");
            return (setup, decode_bincode(&layout_bytes, "CompiledCircuitsSet"));
        }

        let airbender = airbender_dev_path();
        let bin_path = airbender.join("tools/verifier/recursion_in_unified_layer.bin");
        assert!(
            bin_path.exists(),
            "missing Airbender unified verifier binary; set AIRBENDER_DEV_PATH or provide FRI_LAYOUT_PATH"
        );

        let (_, padded_binary_u32) = read_and_pad_binary(&bin_path);
        let layout = get_unified_circuit_artifact_for_machine_type::<
            IWithoutByteAccessIsaConfigWithDelegation,
        >(&padded_binary_u32);

        if let Some(parent) = layout_path.parent() {
            std::fs::create_dir_all(parent).expect("create FRI fixture directory");
        }

        std::fs::write(
            layout_path,
            encode_bincode(&layout, "CompiledCircuitsSet fixture"),
        )
        .expect("write generated FRI layout fixture");

        (setup, layout)
    }

    fn statement_versioned_hash(output: &[u32; 16]) -> B256 {
        use rig::alloy::primitives::keccak256;
        let mut buf = Vec::with_capacity(1 + 16 * 4);
        buf.push(FRI_STATEMENT_HASH_VERSION);
        for word in output.iter() {
            buf.extend_from_slice(&word.to_le_bytes());
        }
        B256::from(keccak256(&buf).0)
    }

    /// Full end-to-end test: real Airbender proof → FRI precompile returns true.
    ///
    /// Loads a gzip-compressed `UnrolledProgramProof` from disk, runs the host-side
    /// unified verifier, derives the `statement_versioned_hash`, then executes a
    /// `FRI_PROOF_TX` inside a gateway-mode block and asserts that the FRI precompile
    /// at 0x0101 returns ABI-encoded `true`.
    #[test]
    fn fri_precompile_returns_true_for_verified_proof() {
        let proof_path = resolve_path(
            "FRI_ORACLE_PATH",
            "matter-labs_b18507c4-50f3-4638-854a-ed625c7e685a_11024243.bin",
        );
        let setup_path = resolve_path(
            "FRI_SETUP_PATH",
            "tests/fixtures/fri/recursion_unified_setup.bin",
        );
        let layout_path = resolve_path(
            "FRI_LAYOUT_PATH",
            "tests/fixtures/fri/recursion_unified_layouts.bin",
        );

        assert!(
            proof_path.exists(),
            "missing proof fixture at {}; set FRI_ORACLE_PATH",
            proof_path.display()
        );

        // ----- 1. Decode proof --------------------------------------------------
        let proof_bytes = std::fs::read(&proof_path).expect("read proof");
        let proof_bytes = maybe_decompress(&proof_bytes);
        let proof: UnrolledProgramProof = decode_bincode(&proof_bytes, "UnrolledProgramProof");

        // ----- 2. Decode setup and compiled circuit layouts ---------------------
        let (setup, compiled_layouts) =
            ensure_recursion_unified_artifacts(&proof, &setup_path, &layout_path);

        // ----- 3. Host-side verification → [u32; 16] output --------------------
        // `verify_proof_in_unified_layer` flattens and runs the verifier in a
        // dedicated thread with a large stack (1 << 27), matching the bootloader.
        let verifier_output = verify_proof_in_unified_layer(
            &proof,
            &setup,
            &compiled_layouts,
            false, // input_is_unrolled = false → unified-over-unified path
        )
        .expect("proof must verify");

        println!("Verifier output: {verifier_output:#010x?}");

        // ----- 4. Derive statement_versioned_hash and oracle stream -------------
        let stmt_hash = statement_versioned_hash(&verifier_output);
        println!("statement_versioned_hash: {stmt_hash}");

        let oracle_stream = flatten_proof_into_responses_for_unified_recursion(
            &proof,
            &setup,
            &compiled_layouts,
            false,
        );

        // ----- 5. Set up gateway-mode block with the proof sidecar --------------
        let stmt_bytes32 = rig::zk_ee::utils::Bytes32::from_array(stmt_hash.0);
        let mut tester = TestingFramework::new()
            .with_mock_fri_sidecars([(stmt_bytes32, oracle_stream)]);
        let wallet = tester.prefunded_random_signer();

        // ----- 6. Submit FRI_PROOF_TX that calls the FRI precompile -------------
        //
        // The FRI statement state is tx-scoped and cleared by `finish_tx()`.  The
        // only way to observe it is from within the *same* transaction: the
        // FRI_PROOF_TX verifies the proof during pre-execution, then calls the
        // precompile (via `to` + `input`) in its execution body.  The precompile
        // reads the tx-scoped state and returns ABI-encoded true.
        let unsigned = UnsignedZKsyncFriProofTx {
            chain_id: 37,
            nonce: 0,
            max_priority_fee_per_gas: 1_000,
            max_fee_per_gas: 25_000,
            gas_limit: 5_000_000,
            // Call the FRI precompile with the statement hash as input.
            to: FRI_PRECOMPILE,
            value: Default::default(),
            input: stmt_hash.0.to_vec().into(),
            access_list: AccessList::default(),
            statement_versioned_hashes: vec![stmt_hash],
        };
        let signed = unsigned.sign(wallet);
        let fri_tx = ZKsyncTxEnvelope::FriProof(signed);

        let output = tester.execute_block(vec![fri_tx]);

        // The FRI_PROOF_TX must succeed and the precompile must return true.
        assert!(
            tx_succeeded(&output, 0),
            "FRI_PROOF_TX must succeed, got: {:?}",
            output.tx_results[0]
        );
        let result = output.tx_results[0].as_ref().unwrap();
        match &result.execution_result {
            rig::zksync_os_interface::types::ExecutionResult::Success(ExecutionOutput::Call(
                data,
            )) => {
                assert_eq!(data.len(), 32, "FRI precompile must return 32 bytes");
                let mut expected = [0u8; 32];
                expected[31] = 1;
                assert_eq!(
                    data.as_slice(),
                    &expected,
                    "FRI precompile must return ABI-encoded true for verified statement hash"
                );
            }
            other => panic!("expected success with true return, got: {other:?}"),
        }
    }
}
