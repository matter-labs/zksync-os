//!
//! These tests are focused on system hooks functionality.
//!
#![cfg(test)]

use alloy_sol_types::{sol, SolEvent};
use rig::alloy::primitives::address;
use rig::alloy::primitives::Address;
use rig::evm_bytecode;
use rig::forward_system::system::tracers::call_tracer::CallTracer;
use rig::ruint::aliases::B160;
use rig::ruint::aliases::U256;
use rig::system_hooks::addresses_constants::L2_INTEROP_ROOT_STORAGE_ADDRESS;
use rig::system_hooks::addresses_constants::SYSTEM_CONTEXT_ADDRESS;
use rig::testing_utils::{call_address_and_measure_gas_cost, get_first_traced_call_to};
use rig::tx_failed;
use rig::tx_succeeded;
use rig::utils::{
    address_into_special_storage_key, AccountProperties, L1TxBuilder,
    ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use rig::zk_ee::system::validator::NopTxValidator;
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

/// COMPLEX_UPGRADER (0x800f) is an authorized caller for set_bytecode_on_address (0x7002).
#[test]
fn test_set_bytecode_on_address_from_complex_upgrader() {
    let complex_upgrader_address = address!("000000000000000000000000000000000000800f");
    let set_bytecode_hook_address = address!("0000000000000000000000000000000000007002");

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
            complex_upgrader_address,
            U256::from(1_000_000_000_000_000_u64),
        );

    let tx = L1TxBuilder::new()
        .from(complex_upgrader_address)
        .to(set_bytecode_hook_address)
        .input(calldata)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

    assert!(
        tx_succeeded(&output, 0),
        "COMPLEX_UPGRADER must be authorized to call set_bytecode_on_address"
    );

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

    // Gas charged by the L1Messenger system contract's EVM bytecode (keccak + LOG costs).
    // The hook itself charges 0 ergs.
    assert_eq!(gas_used, 9202);
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

    assert_eq!(gas_used, 52359);
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
    assert_eq!(gas_used, 54392);
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

/// Measures the gas cost of BALANCE on `target` by deploying a probe contract,
/// executing it, and reading the stored gas measurement from slot 0.
fn measure_balance_gas_cost(target: Address) -> u64 {
    measure_balance_gas_cost_inner(target, false)
}

/// Same as [`measure_balance_gas_cost`] but skips the REVM consistency check.
fn measure_balance_gas_cost_without_revm_check(target: Address) -> u64 {
    measure_balance_gas_cost_inner(target, true)
}

fn measure_balance_gas_cost_inner(target: Address, skip_revm_check: bool) -> u64 {
    let probe_address = address!("cccccccccccccccccccccccccccccccccccccccc");
    let sender = address!("dddddddddddddddddddddddddddddddddddddddd");
    let bytecode = evm_bytecode::balance_gas_probe(target);

    let mut tester = TestingFramework::new()
        .with_evm_contract(probe_address, &bytecode)
        .with_balance(
            sender,
            alloy::primitives::U256::from(1_000_000_000_000_000_u64),
        );
    if skip_revm_check {
        tester = tester.without_revm_consistency_check();
    }

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(probe_address)
        .input(Vec::new())
        .gas_price(1000)
        .gas_limit(200_000)
        .nonce(0)
        .build();

    let output = tester.execute_block(vec![tx]);
    assert!(tx_succeeded(&output, 0), "probe tx must succeed");

    let slot = tester
        .get_storage_slot(&probe_address, U256::ZERO)
        .expect("slot 0 must be written");
    slot.into_u256_be().as_limbs()[0]
}

/// EVM precompiles (0x01..0x0a) must be warm at transaction start.
/// System hook addresses (0x7001, 0x7002, 0x7100) must be cold.
#[test]
fn test_precompiles_warm_hooks_cold_at_tx_start() {
    // Overhead between the two GAS snapshots: PUSH20(3) + POP(2) + GAS(2) = 7
    const OVERHEAD: u64 = 7;
    const WARM_BALANCE: u64 = 100 + OVERHEAD;
    const COLD_BALANCE: u64 = 2600 + OVERHEAD;

    // EVM precompiles should be warm
    let ecrecover = address!("0000000000000000000000000000000000000001");
    let sha256 = address!("0000000000000000000000000000000000000002");
    let identity = address!("0000000000000000000000000000000000000004");

    assert_eq!(
        measure_balance_gas_cost(ecrecover),
        WARM_BALANCE,
        "ecrecover (0x01) must be warm"
    );
    assert_eq!(
        measure_balance_gas_cost(sha256),
        WARM_BALANCE,
        "sha256 (0x02) must be warm"
    );
    assert_eq!(
        measure_balance_gas_cost(identity),
        WARM_BALANCE,
        "identity (0x04) must be warm"
    );

    // System hook addresses should be cold (these were incorrectly warmed before the A1 fix)
    let l1_messenger_hook = address!("0000000000000000000000000000000000007001");
    let set_bytecode_hook = address!("0000000000000000000000000000000000007002");
    let mint_hook = address!("0000000000000000000000000000000000007100");
    let contract_deployer = address!("0000000000000000000000000000000000008006");

    assert_eq!(
        measure_balance_gas_cost(l1_messenger_hook),
        COLD_BALANCE,
        "l1_messenger hook (0x7001) must be cold"
    );
    assert_eq!(
        measure_balance_gas_cost(set_bytecode_hook),
        COLD_BALANCE,
        "set_bytecode hook (0x7002) must be cold"
    );
    assert_eq!(
        measure_balance_gas_cost(mint_hook),
        COLD_BALANCE,
        "mint hook (0x7100) must be cold"
    );
    assert_eq!(
        measure_balance_gas_cost(contract_deployer),
        COLD_BALANCE,
        "contract_deployer hook (0x8006) must be cold"
    );

    // blake2f (0x09) is not enabled in test/production builds, so it must be cold
    let blake2f = address!("0000000000000000000000000000000000000009");
    assert_eq!(
        measure_balance_gas_cost(blake2f),
        COLD_BALANCE,
        "blake2f (0x09) must be cold"
    );

    // point_eval (0x0a) is enabled via eip-4844 feature in test builds, so it is warm here.
    // Note: in production builds point_eval is NOT enabled, so it would be cold there.
    // Skip REVM consistency check because REVM does not warm point_eval.
    let point_eval = address!("000000000000000000000000000000000000000a");
    assert_eq!(
        measure_balance_gas_cost_without_revm_check(point_eval),
        WARM_BALANCE,
        "point_eval (0x0a) must be warm (enabled via eip-4844 in test builds)"
    );
}

/// L1 messenger hook must not charge EVM gas (ergs) even for authorized calls.
/// This enforces the "indistinguishable from a call to an empty account" invariant.
#[test]
fn test_l1_messenger_hook_authorized_no_ergs_charge() {
    let l1_messenger_contract = address!("0000000000000000000000000000000000008008");
    let l1_messenger_hook = address!("0000000000000000000000000000000000007001");

    // Valid calldata: abi.encodePacked(address msg.sender, bytes message)
    let hook_calldata = hex::decode(
        "000000000000000000000000111111111111111111111111111111111111111100000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000020000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    .unwrap();

    let mut tester = TestingFramework::new().with_system_contracts(false, false);

    let tx = L1TxBuilder::new()
        .from(l1_messenger_contract)
        .to(l1_messenger_hook)
        .input(hook_calldata)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let mut tracer = CallTracer::default();
    let mut nop_validator = NopTxValidator;
    let output = tester.execute_block_with_tracing(vec![tx], &mut tracer, &mut nop_validator);

    assert!(
        tx_succeeded(&output, 0),
        "authorized L1 messenger hook call must succeed"
    );

    let call =
        get_first_traced_call_to(l1_messenger_hook, &tracer).expect("call to hook must be traced");
    assert_eq!(
        call.gas_used, 0,
        "L1 messenger hook must not charge EVM gas (ergs)"
    );
}
