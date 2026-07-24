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
fn test_interop_commitment_leaf_hook_succeeds() {
    // making sure hooks are installed
    let mut tester = TestingFramework::new().with_system_contracts(false, false);

    let interop_commitment_tree_contract = address!("0000000000000000000000000000000000010012");

    let interop_commitment_leaf_hook = address!("0000000000000000000000000000000000007004");

    // Calldata is exactly the 32-byte leaf hash
    let leaf_hash = [0xabu8; 32];

    let tx = L1TxBuilder::new()
        .from(interop_commitment_tree_contract)
        .to(interop_commitment_leaf_hook)
        .input(leaf_hash.to_vec())
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

    let tx_output = output.tx_results.first().unwrap().as_ref().unwrap();

    match &tx_output.execution_result {
        ExecutionResult::Success(_) => {
            // ok
        }
        tx_result => {
            panic!(
                "interop commitment leaf hook call from authorized sender did not succeed: {tx_result:?}"
            );
        }
    }

    let leaf_log = tx_output
        .l2_to_l1_logs
        .iter()
        .find(|log_with_preimage| log_with_preimage.log.sender == interop_commitment_tree_contract)
        .expect("interop commitment leaf log must be emitted");
    assert_eq!(
        leaf_log.log.value.0, leaf_hash,
        "leaf log value must be the leaf hash"
    );
    assert_eq!(
        leaf_log.log.key,
        alloy::primitives::B256::ZERO,
        "leaf log key must be zero"
    );
    assert!(leaf_log.log.is_service);
    assert!(
        leaf_log.preimage.is_none(),
        "leaf log must not carry a preimage"
    );
}

#[test]
fn test_interop_commitment_leaf_hook_fails_with_invalid_calldata() {
    // making sure hooks are installed
    let mut tester = TestingFramework::new().with_system_contracts(false, false);

    let interop_commitment_tree_contract = address!("0000000000000000000000000000000000010012");

    let interop_commitment_leaf_hook = address!("0000000000000000000000000000000000007004");

    // Invalid calldata: not exactly 32 bytes
    let hook_calldata = vec![0xabu8; 31];

    let tx = L1TxBuilder::new()
        .from(interop_commitment_tree_contract)
        .to(interop_commitment_leaf_hook)
        .input(hook_calldata)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

    let tx_output = output.tx_results.first().unwrap().as_ref().unwrap();

    assert!(matches!(
        tx_output.execution_result,
        ExecutionResult::Revert { .. }
    ));
    assert!(
        tx_output
            .l2_to_l1_logs
            .iter()
            .all(|log_with_preimage| log_with_preimage.log.sender
                != interop_commitment_tree_contract),
        "no interop commitment leaf log must be emitted on revert"
    );
}

#[test]
fn test_interop_commitment_leaf_hook_unauthorized_sender_ignored() {
    // making sure hooks are installed
    let mut tester = TestingFramework::new().with_system_contracts(false, false);

    // ❌ this should NOT be the L2InteropCommitmentTree system contract address
    let unauthorized_from = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

    let interop_commitment_tree_contract = address!("0000000000000000000000000000000000010012");

    let interop_commitment_leaf_hook = address!("0000000000000000000000000000000000007004");

    let hook_calldata = [0xabu8; 32].to_vec();

    let tx = L1TxBuilder::new()
        .from(unauthorized_from)
        .to(interop_commitment_leaf_hook)
        .input(hook_calldata.clone())
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);

    let tx_output = output.tx_results.first().unwrap().as_ref().unwrap();

    match &tx_output.execution_result {
        ExecutionResult::Success(ExecutionOutput::Call(return_data)) => {
            assert!(
                return_data.is_empty(),
                "unauthorized call must return empty data"
            );
        }
        tx_result => panic!("unauthorized call must succeed as empty account, got: {tx_result:?}"),
    }

    assert!(
        tx_output
            .l2_to_l1_logs
            .iter()
            .all(|log_with_preimage| log_with_preimage.log.sender
                != interop_commitment_tree_contract),
        "unauthorized caller must not emit an interop commitment leaf log"
    );

    let gas_used = call_address_and_measure_gas_cost(
        interop_commitment_leaf_hook,
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
    let sender = address!("1234567890123456789012345678901234567890");
    for test_contract_address in [L2_INTEROP_ROOT_STORAGE_ADDRESS, SYSTEM_CONTEXT_ADDRESS] {
        // Contract that emits a log with empty topics array - this should be handled gracefully
        let test_contract = Address::from(test_contract_address.to_be_bytes());

        // Default stub emits LOG0 (no topics). For SystemContext, keep that behavior
        // on the test's empty-calldata call, but return slot 0 for non-empty
        // calldata so currentSettlementLayerChainId() still works.
        let test_contract_bytecode = if test_contract_address == SYSTEM_CONTEXT_ADDRESS {
            hex::decode("365f14600e575f545f5260205ff35b60006000a000").unwrap()
        } else {
            hex::decode("60006000a000").unwrap()
        };
        let mut tester =
            TestingFramework::new().with_evm_contract(test_contract, &test_contract_bytecode);

        let tx = L1TxBuilder::new()
            .from(sender)
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
//   3. FRI_PROOF_TX_TYPE is rejected when chain-config FRI support is disabled.
#[cfg(feature = "fri_precompile")]
mod fri_precompile {
    use super::*;
    use rig::alloy::eips::eip2930::AccessList;
    use rig::alloy::primitives::B256;
    use rig::zksync_os_interface::types::ExecutionOutput;
    use zksync_os_tests_common::zksync_tx::{
        fri_proof_tx::UnsignedZKsyncFriProofTx, ZKsyncTxEnvelope,
    };

    // The FRI precompile lives at address 0x0000...7003 (FRI_PRECOMPILE_ADDRESS_LOW = 0x7003).
    const FRI_PRECOMPILE: Address = address!("0000000000000000000000000000000000007003");

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
        // The FRI sidecar is empty by default, but the tx should be
        // rejected before sidecar resolution because chain-config FRI support is disabled.
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
// The test reads a gzip-compressed `UnrolledProgramProof` fixture from disk,
// hands the raw (decompressed) bincode bytes to the Gateway-side FRI sidecar,
// and configures `FriVerifierArtifacts` on the oracle side so the bootloader's
// `FRI_PROOF_QUERY_ID` responder decodes and flattens the proof for the
// airbender unified verifier.
//
// When the proof fixture is missing the test is a silent no-op: the path is
// set via FRI_ORACLE_PATH (default points at a committed fixture in the repo
// root). Set AIRBENDER_DEV_PATH or provide FRI_LAYOUT_PATH to control the
// source of the compiled circuit layouts when regenerating fixtures.
// ---------------------------------------------------------------------------
#[cfg(feature = "fri_precompile")]
mod fri_precompile_e2e {
    use super::*;
    use rig::alloy::eips::eip2930::AccessList;
    use rig::alloy::primitives::{B256, U256 as AlloyU256};
    use rig::zksync_os_interface::types::ExecutionOutput;
    use zksync_os_tests_common::zksync_tx::{
        fri_proof_tx::UnsignedZKsyncFriProofTx, ZKsyncTxEnvelope,
    };

    use alloy_sol_types::SolCall;
    use execution_utils::setups::{read_and_pad_binary, CompiledCircuitsSet};
    use execution_utils::unified_circuit::{
        get_unified_circuit_artifact_for_machine_type, verify_proof_in_unified_layer,
    };
    use execution_utils::unrolled::{UnrolledProgramProof, UnrolledProgramSetup};
    use flate2::read::GzDecoder;
    use full_statement_verifier::verifier_common::SecurityModel;
    use rig::forward_system::run::FriVerifierArtifacts;
    use rig::forward_system::system::system_types::ForwardRunningSystem;
    use rig::forward_system::system::tracers::precompile_stats::PrecompileStatsTracer;
    use rig::zk_ee::system::validator::NopTxValidator;
    use riscv_transpiler::cycle::IWithoutByteAccessIsaConfigWithDelegation;
    use std::io::Read;
    use std::path::PathBuf;

    pub(super) const FRI_STATEMENT_HASH_VERSION: u8 = 1;
    // Address 0x7003 — the FRI precompile.
    const FRI_PRECOMPILE: Address = address!("0000000000000000000000000000000000007003");
    const FRI_VERIFIER_CONTRACT: Address = address!("000000000000000000000000000000000000f101");
    // Runtime bytecode compiled from
    // contracts/l1-contracts/contracts/state-transition/verifiers/ZKsyncOSVerifierFri.sol
    // in zksync-era-clean-latest.
    const FRI_VERIFIER_DEPLOYED_BYTECODE: &str =
        include_str!("../../../fixtures/fri/zksync_os_verifier_fri_deployed_bytecode.hex");
    pub(super) const FRI_PRIMARY_PROOF_FIXTURE: &str =
        "tests/fixtures/fri/fri_proof_security_100_14470757.bin";
    const FRI_SECONDARY_PROOF_FIXTURE: &str =
        "tests/fixtures/fri/fri_proof_security_100_14469803.bin";

    sol! {
        interface ZKsyncOSVerifierFri {
            function verify(uint256[] calldata _publicInputs, uint256[] calldata _proof) external view returns (bool);
        }
    }

    fn repo_root() -> PathBuf {
        // CARGO_MANIFEST_DIR = tests/instances/system_hooks
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repo root is three directories above the crate manifest")
            .to_path_buf()
    }

    pub(super) fn resolve_path(env_var: &str, default_relative: &str) -> PathBuf {
        if let Ok(v) = std::env::var(env_var) {
            PathBuf::from(v)
        } else {
            repo_root().join(default_relative)
        }
    }

    pub(super) fn maybe_decompress(bytes: &[u8]) -> Vec<u8> {
        let decoded = if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut out = Vec::new();
            GzDecoder::new(bytes)
                .read_to_end(&mut out)
                .expect("gzip decompress");
            out
        } else {
            bytes.to_vec()
        };

        // Strip the 10-byte EPROOF01 server envelope (8-byte magic + 1-byte
        // version + 1-byte security-bits marker) when present, leaving the raw
        // bincoded `UnrolledProgramProof` the verifier and sidecar consumers
        // both expect.
        if decoded.starts_with(b"EPROOF01") && decoded.len() > 10 {
            decoded[10..].to_vec()
        } else {
            decoded
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
        let mut buf = Vec::with_capacity(16 * 4);
        for word in output.iter() {
            buf.extend_from_slice(&word.to_le_bytes());
        }
        let mut hash = keccak256(&buf).0;
        hash[0] = FRI_STATEMENT_HASH_VERSION;
        B256::from(hash)
    }

    fn fri_gateway_run_config() -> rig::chain::RunConfig {
        let mut run_config = rig::chain::RunConfig::default();
        // The benchmark/test still runs the RISC-V simulator when enabled by
        // the environment. This disables only the legacy extra assertion that
        // assumes the guest returns the storage-diff hash. The current
        // `for_tests` proving binary returns the batch public-input hash.
        run_config.check_storage_diff_hashes = false;
        run_config
    }

    /// Loads a committed proof fixture and returns `(raw_bincode_bytes, stmt_hash)`.
    ///
    /// Simulates the submitter side: decompress the on-disk fixture, decode
    /// it, run the host-side verifier to derive `statement_versioned_hash`.
    /// Returns `None` when the fixture is absent so callers can silent-skip.
    pub(super) fn load_proof_fixture(
        fixture_relative: &str,
        setup_path: &PathBuf,
        layout_path: &PathBuf,
    ) -> Option<(Vec<u8>, B256)> {
        let proof_path = resolve_path("FRI_ORACLE_PATH_UNUSED", fixture_relative);
        if !proof_path.exists() {
            eprintln!(
                "skipping FRI test: missing fixture {}",
                proof_path.display()
            );
            return None;
        }

        let fixture_bytes = std::fs::read(&proof_path).expect("read proof");
        let proof_bytes = maybe_decompress(&fixture_bytes);
        let proof: UnrolledProgramProof = decode_bincode(&proof_bytes, "UnrolledProgramProof");
        let (setup, compiled_layouts) =
            ensure_recursion_unified_artifacts(&proof, setup_path, layout_path);
        let verifier_output = verify_proof_in_unified_layer(
            &proof,
            &setup,
            &compiled_layouts,
            false,
            SecurityModel::Security100,
        )
        .expect("proof must verify");
        let stmt_hash = statement_versioned_hash(&verifier_output);
        Some((proof_bytes, stmt_hash))
    }

    fn load_proof_fixture_with_output(
        fixture_relative: &str,
        setup_path: &PathBuf,
        layout_path: &PathBuf,
    ) -> Option<(Vec<u8>, [u32; 16])> {
        let proof_path = resolve_path("FRI_ORACLE_PATH", fixture_relative);
        if !proof_path.exists() {
            eprintln!(
                "skipping FRI verifier contract test: missing fixture {}",
                proof_path.display()
            );
            return None;
        }

        let fixture_bytes = std::fs::read(&proof_path).expect("read proof");
        let proof_bytes = maybe_decompress(&fixture_bytes);
        let proof: UnrolledProgramProof = decode_bincode(&proof_bytes, "UnrolledProgramProof");
        let (setup, compiled_layouts) =
            ensure_recursion_unified_artifacts(&proof, setup_path, layout_path);
        let verifier_output = verify_proof_in_unified_layer(
            &proof,
            &setup,
            &compiled_layouts,
            false,
            SecurityModel::Security100,
        )
        .expect("proof must verify");
        Some((proof_bytes, verifier_output))
    }

    pub(super) fn default_setup_and_layout_paths() -> (PathBuf, PathBuf) {
        let setup_path = resolve_path(
            "FRI_SETUP_PATH",
            "tests/fixtures/fri/recursion_unified_setup.bin",
        );
        let layout_path = resolve_path(
            "FRI_LAYOUT_PATH",
            "tests/fixtures/fri/recursion_unified_layouts.bin",
        );
        (setup_path, layout_path)
    }

    pub(super) fn load_verifier_artifacts(
        setup_path: &PathBuf,
        layout_path: &PathBuf,
    ) -> Option<FriVerifierArtifacts> {
        if !setup_path.exists() || !layout_path.exists() {
            return None;
        }
        let setup_bytes = std::fs::read(setup_path).expect("read setup");
        let layout_bytes = std::fs::read(layout_path).expect("read layout");
        Some(FriVerifierArtifacts {
            setup: decode_bincode(&setup_bytes, "UnrolledProgramSetup"),
            compiled_layouts: decode_bincode(&layout_bytes, "CompiledCircuitsSet"),
        })
    }

    /// Full end-to-end test: real Airbender proof → FRI precompile returns true.
    ///
    /// Loads a gzip-compressed `UnrolledProgramProof` fixture from disk, hands
    /// the raw (decompressed) bincode bytes to the Gateway-side FRI sidecar,
    /// and configures `FriVerifierArtifacts` on the oracle so the bootloader
    /// resolves `FRI_PROOF_QUERY_ID` into the flattened word stream. The test
    /// then executes a `FRI_PROOF_TX` inside a gateway-mode block and asserts
    /// that the FRI precompile at 0x7003 returns ABI-encoded `true`.
    ///
    /// Acts as a silent no-op when the proof fixture is not present on disk.
    #[test]
    fn fri_precompile_returns_true_for_verified_proof() {
        let proof_path = resolve_path("FRI_ORACLE_PATH", FRI_PRIMARY_PROOF_FIXTURE);
        let setup_path = resolve_path(
            "FRI_SETUP_PATH",
            "tests/fixtures/fri/recursion_unified_setup.bin",
        );
        let layout_path = resolve_path(
            "FRI_LAYOUT_PATH",
            "tests/fixtures/fri/recursion_unified_layouts.bin",
        );

        if !proof_path.exists() {
            eprintln!(
                "skipping fri_precompile_returns_true_for_verified_proof: missing fixture {}",
                proof_path.display()
            );
            return;
        }

        // ----- 1. Load raw proof bytes (simulate what the sequencer receives)
        // The sidecar is a dumb byte store: the bootloader's FRI oracle does
        // the bincode decode and flattening. The fixture on disk is gzipped
        // for size, but on the wire the operator hands raw bincode bytes.
        let fixture_bytes = std::fs::read(&proof_path).expect("read proof");
        let proof_bytes = maybe_decompress(&fixture_bytes);

        // ----- 2. Decode setup and compiled circuit layouts (test-only) -------
        // The submitter runs the host-side verifier off-chain to derive
        // `statement_versioned_hash`; they have their own prover stack and do
        // not depend on zksync-os. We simulate that here.
        let proof: UnrolledProgramProof = decode_bincode(&proof_bytes, "UnrolledProgramProof");
        let (setup, compiled_layouts) =
            ensure_recursion_unified_artifacts(&proof, &setup_path, &layout_path);

        // ----- 3. Host-side verification → [u32; 16] output -------------------
        // `verify_proof_in_unified_layer` flattens and runs the verifier in a
        // dedicated thread with a large stack (1 << 27), matching the bootloader.
        let verifier_output = verify_proof_in_unified_layer(
            &proof,
            &setup,
            &compiled_layouts,
            false, // input_is_unrolled = false → unified-over-unified path
            SecurityModel::Security100,
        )
        .expect("proof must verify");

        // ----- 4. Derive statement_versioned_hash -----------------------------
        let stmt_hash = statement_versioned_hash(&verifier_output);

        // ----- 5. Set up gateway-mode block with raw proof bytes + artifacts --
        let stmt_bytes32 = rig::zk_ee::utils::Bytes32::from_array(stmt_hash.0);
        let (mut tester, _counter) = TestingFramework::new().with_mock_fri_sidecars(
            [(stmt_bytes32, proof_bytes)],
            Some(FriVerifierArtifacts {
                setup,
                compiled_layouts,
            }),
        );
        tester = tester.with_run_config(fri_gateway_run_config());
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

    /// Real proof + compiled Solidity verifier bytecode:
    ///
    /// The tx first verifies the FRI sidecar through the bootloader path, then
    /// executes `ZKsyncOSVerifierFri.verify(...)`. The contract recomputes the
    /// statement hash from the proof arguments and reaches the FRI precompile
    /// at 0x7003. A `true` return proves the contract bytecode, ABI shape,
    /// tx-scoped verified-statement state, and precompile all line up.
    fn execute_fri_verifier_contract_tx(
        fixture_relative: &str,
        precompile_stats: Option<&mut PrecompileStatsTracer<ForwardRunningSystem>>,
    ) -> Option<rig::BlockOutput> {
        let (setup_path, layout_path) = default_setup_and_layout_paths();
        let verifier_bytecode = hex::decode(FRI_VERIFIER_DEPLOYED_BYTECODE.trim())
            .expect("decode FRI verifier bytecode");
        let Some((proof_bytes, verifier_output)) =
            load_proof_fixture_with_output(fixture_relative, &setup_path, &layout_path)
        else {
            return None;
        };

        let stmt_hash = statement_versioned_hash(&verifier_output);
        let words_to_u256 = |words: &[u32]| {
            let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
            AlloyU256::from_be_slice(&bytes)
        };
        let public_input_hash = words_to_u256(&verifier_output[..8]);

        let call = ZKsyncOSVerifierFri::verifyCall {
            _publicInputs: vec![public_input_hash >> 32],
            _proof: vec![public_input_hash],
        };

        let artifacts = load_verifier_artifacts(&setup_path, &layout_path)
            .expect("artifacts must be present if fixture verification succeeded");
        let stmt_bytes32 = rig::zk_ee::utils::Bytes32::from_array(stmt_hash.0);
        let (mut tester, _counter) = TestingFramework::new()
            .with_mock_fri_sidecars([(stmt_bytes32, proof_bytes)], Some(artifacts));
        tester = tester.with_run_config(fri_gateway_run_config());
        tester.set_evm_contract(FRI_VERIFIER_CONTRACT, &verifier_bytecode);
        let wallet = tester.prefunded_random_signer();

        let unsigned = UnsignedZKsyncFriProofTx {
            chain_id: 37,
            nonce: 0,
            max_priority_fee_per_gas: 1_000,
            max_fee_per_gas: 25_000,
            gas_limit: 10_000_000,
            to: FRI_VERIFIER_CONTRACT,
            value: Default::default(),
            input: call.abi_encode().into(),
            access_list: AccessList::default(),
            statement_versioned_hashes: vec![stmt_hash],
        };
        let signed = unsigned.sign(wallet);
        let fri_tx = ZKsyncTxEnvelope::FriProof(signed);

        Some(if let Some(tracer) = precompile_stats {
            tester.execute_block_with_tracing(vec![fri_tx], tracer, &mut NopTxValidator)
        } else {
            tester.execute_block(vec![fri_tx])
        })
    }

    fn assert_fri_verifier_contract_output(output: &rig::BlockOutput) {
        assert!(
            tx_succeeded(output, 0),
            "FRI verifier contract call must succeed, got: {:?}",
            output.tx_results[0]
        );
        let result = output.tx_results[0].as_ref().unwrap();
        match &result.execution_result {
            rig::zksync_os_interface::types::ExecutionResult::Success(ExecutionOutput::Call(
                data,
            )) => {
                let mut expected = [0u8; 32];
                expected[31] = 1;
                assert_eq!(
                    data.as_slice(),
                    &expected,
                    "compiled FRI verifier contract must return ABI-encoded true"
                );
            }
            other => panic!("expected verifier contract success with true return, got: {other:?}"),
        }
    }

    fn precompile_stats_enabled() -> bool {
        std::env::var("PRECOMPILE_STATS_PATH").is_ok()
            || std::env::var("PRECOMPILE_SAMPLES_DIR").is_ok()
    }

    fn dump_precompile_stats(precompile_stats: &PrecompileStatsTracer<ForwardRunningSystem>) {
        precompile_stats.print_stats();

        if let Ok(path) = std::env::var("PRECOMPILE_STATS_PATH") {
            precompile_stats
                .write_csv(std::path::Path::new(&path))
                .expect("write precompile stats");
        }
        if let Ok(dir) = std::env::var("PRECOMPILE_SAMPLES_DIR") {
            precompile_stats
                .dump_samples(std::path::Path::new(&dir))
                .expect("dump precompile samples");
        }
    }

    /// Returns every committed 100-bit FRI proof fixture (file name
    /// → repo-relative path) in deterministic block-number order. Used by
    /// `fri_verifier_contract_returns_true_for_verified_proof` to exercise
    /// the full pipeline across the whole fixture corpus and, when
    /// `cycle_marker` is enabled, to emit one bench sample per proof.
    fn all_fri_proof_fixtures() -> Vec<(u64, String)> {
        let dir = repo_root().join("tests/fixtures/fri");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("read fixtures dir") {
            let entry = entry.expect("read fixtures dir entry");
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(block_str) = name
                .strip_prefix("fri_proof_security_100_")
                .and_then(|s| s.strip_suffix(".bin"))
            else {
                continue;
            };
            let Ok(block) = block_str.parse::<u64>() else {
                continue;
            };
            out.push((block, format!("tests/fixtures/fri/{name}")));
        }
        out.sort_by_key(|(block, _)| *block);
        out
    }

    #[test]
    fn fri_verifier_contract_returns_true_for_verified_proof() {
        let fixtures = all_fri_proof_fixtures();
        assert!(
            !fixtures.is_empty(),
            "no FRI proof fixtures found under tests/fixtures/fri/"
        );

        let mut precompile_stats = PrecompileStatsTracer::<ForwardRunningSystem>::default();
        for (_block, fixture) in &fixtures {
            cycle_marker::log_marker("Params: fri_proof");
            let output = if precompile_stats_enabled() {
                execute_fri_verifier_contract_tx(fixture, Some(&mut precompile_stats))
            } else {
                execute_fri_verifier_contract_tx(fixture, None)
            };
            let Some(output) = output else {
                continue;
            };
            assert_fri_verifier_contract_output(&output);
        }
        if precompile_stats_enabled() {
            dump_precompile_stats(&precompile_stats);
        }
    }

    /// E2E: a single `FRI_PROOF_TX` that carries two distinct verified
    /// statement hashes. Both must verify, both must land on the
    /// tx-level metadata, and the precompile must find both when
    /// queried. We query the second hash (not the first) as calldata to
    /// prove the predicate looks past position 0.
    #[test]
    fn fri_precompile_finds_any_of_multiple_verified_proofs() {
        let (setup_path, layout_path) = default_setup_and_layout_paths();

        let Some((proof_bytes_1, stmt_hash_1)) =
            load_proof_fixture(FRI_PRIMARY_PROOF_FIXTURE, &setup_path, &layout_path)
        else {
            return;
        };
        let Some((proof_bytes_2, stmt_hash_2)) =
            load_proof_fixture(FRI_SECONDARY_PROOF_FIXTURE, &setup_path, &layout_path)
        else {
            return;
        };
        assert_ne!(
            stmt_hash_1, stmt_hash_2,
            "fixtures must produce distinct statement hashes to exercise the multi-hash path"
        );

        let artifacts = load_verifier_artifacts(&setup_path, &layout_path)
            .expect("artifacts must be present if fixtures are");

        let h1 = rig::zk_ee::utils::Bytes32::from_array(stmt_hash_1.0);
        let h2 = rig::zk_ee::utils::Bytes32::from_array(stmt_hash_2.0);

        let (mut tester, _counter) = TestingFramework::new()
            .with_mock_fri_sidecars([(h1, proof_bytes_1), (h2, proof_bytes_2)], Some(artifacts));
        let wallet = tester.prefunded_random_signer();

        // `input: stmt_hash_2.0` asks the precompile about the second
        // hash (position 1 in the Vec), proving `contains` scans past
        // index 0.
        let unsigned = UnsignedZKsyncFriProofTx {
            chain_id: 37,
            nonce: 0,
            max_priority_fee_per_gas: 1_000,
            max_fee_per_gas: 25_000,
            gas_limit: 5_000_000,
            to: FRI_PRECOMPILE,
            value: Default::default(),
            input: stmt_hash_2.0.to_vec().into(),
            access_list: AccessList::default(),
            statement_versioned_hashes: vec![stmt_hash_1, stmt_hash_2],
        };
        let signed = unsigned.sign(wallet);
        let fri_tx = ZKsyncTxEnvelope::FriProof(signed);

        let output = tester.execute_block(vec![fri_tx]);

        assert!(
            tx_succeeded(&output, 0),
            "multi-hash FRI_PROOF_TX must succeed, got: {:?}",
            output.tx_results[0]
        );
        let result = output.tx_results[0].as_ref().unwrap();
        match &result.execution_result {
            rig::zksync_os_interface::types::ExecutionResult::Success(ExecutionOutput::Call(
                data,
            )) => {
                let mut expected = [0u8; 32];
                expected[31] = 1;
                assert_eq!(
                    data.as_slice(),
                    &expected,
                    "precompile must return true for stmt_hash_2, even though it is not the first hash pushed"
                );
            }
            other => panic!("expected success with true return, got: {other:?}"),
        }
    }

    // NOTE: previously this module also contained negative FRI tests
    // (`fri_proof_tx_dropped_when_any_proof_missing`,
    // `fri_proof_tx_rejects_statement_hash_mismatch`,
    // `fri_proof_tx_rejects_corrupted_proof_bytes`) that asserted the
    // sequencer would drop txs with bad FRI proofs. Those properties
    // are no longer the sequencer's job: under the current design
    // (`VERIFY_FRI_PROOFS = false` on forward-mode configs) the
    // sequencer trusts the admission layer and doesn't run the
    // verifier on its own. The airbender unified verifier (prover)
    // is the final authority. Moving those negative cases to rig-level
    // coverage requires wiring proving-mode execution into the rig,
    // which is out of scope for this PR.

    /// Pins the validation order: cheap structural checks (here:
    /// nonce) run BEFORE FRI-specific validation. On proving-config
    /// runs, FRI verification is still the most expensive validation
    /// step (up to 8 airbender unified verifier runs); on forward-config
    /// runs it's trivial, but the ordering still matters because
    /// `build_verified_fri_statements_list` installs state on
    /// `TxLevelMetadata` and we do not want that to happen for a tx
    /// that will be dropped anyway.
    ///
    /// We assert this by constructing a FRI_PROOF_TX that is
    /// malformed on two dimensions — wrong nonce AND a statement
    /// hash the sidecar doesn't know about (irrelevant on the
    /// sequencer path, would reject on the proving path) — and
    /// confirming the surfaced error is the nonce error.
    #[test]
    fn fri_verification_runs_after_nonce_check() {
        use rig::zksync_os_interface::error::InvalidTransaction;

        let missing_stmt_hash = B256::from([0xffu8; 32]);

        let (mut tester, _counter) = TestingFramework::new().with_mock_fri_sidecars(
            std::iter::empty::<(rig::zk_ee::utils::Bytes32, Vec<u8>)>(),
            None,
        );
        let wallet = tester.prefunded_random_signer();

        // Account nonce at tx start is 0; we submit nonce=7 so the
        // nonce check fails with NonceTooHigh.
        let unsigned = UnsignedZKsyncFriProofTx {
            chain_id: 37,
            nonce: 7,
            max_priority_fee_per_gas: 1_000,
            max_fee_per_gas: 25_000,
            gas_limit: 5_000_000,
            to: FRI_PRECOMPILE,
            value: Default::default(),
            input: missing_stmt_hash.0.to_vec().into(),
            access_list: AccessList::default(),
            statement_versioned_hashes: vec![missing_stmt_hash],
        };
        let signed = unsigned.sign(wallet);
        let fri_tx = ZKsyncTxEnvelope::FriProof(signed);

        let output = tester.execute_block(vec![fri_tx]);

        assert!(
            matches!(
                output.tx_results[0],
                Err(InvalidTransaction::NonceTooHigh { .. })
            ),
            "tx with bogus nonce and bad FRI proof must surface the nonce \
             error, not the FRI error — FRI verification must run AFTER \
             nonce check to avoid DoS amplification; got: {:?}",
            output.tx_results[0]
        );
    }

    /// Pins the validator-level dedup policy for duplicate FRI
    /// statement hashes within a single `FriProofTx`:
    ///
    ///   - The submitter pays for N slots as submitted (raw count
    ///     drives `fri_proof_intrinsic_native_cost` and the per-tx
    ///     `MAX_FRI_STATEMENTS_PER_TX` cap).
    ///   - The validator skips re-verifying a hash it has already
    ///     verified within the same tx — the precompile's membership
    ///     check treats duplicates identically, so extra runs carry
    ///     no information.
    ///   - `TxLevelMetadata.verified_fri_statements` holds each hash
    ///     at most once.
    ///
    /// We observe the dedup indirectly: each actual verifier run
    /// triggers one sidecar lookup. The rig executes each block in
    /// two forward passes (result-keeper + prover-input), so one
    /// unique hash verified per pass gives a counter of 2. Without
    /// dedup, `vec![h, h]` would verify twice per pass (counter = 4).
    #[test]
    fn fri_proof_tx_dedups_duplicate_statement_hashes() {
        let (setup_path, layout_path) = default_setup_and_layout_paths();
        let Some((proof_bytes, stmt_hash)) =
            load_proof_fixture(FRI_PRIMARY_PROOF_FIXTURE, &setup_path, &layout_path)
        else {
            return;
        };
        let artifacts = load_verifier_artifacts(&setup_path, &layout_path)
            .expect("artifacts must be present if fixture is");

        let stmt_bytes32 = rig::zk_ee::utils::Bytes32::from_array(stmt_hash.0);
        let (mut tester, sidecar_lookups) = TestingFramework::new()
            .with_mock_fri_sidecars([(stmt_bytes32, proof_bytes)], Some(artifacts));
        let wallet = tester.prefunded_random_signer();

        // Same hash listed twice: the submitter pays for 2 slots, the
        // validator must run the verifier only once (the second slot
        // finds the hash already present and skips).
        let unsigned = UnsignedZKsyncFriProofTx {
            chain_id: 37,
            nonce: 0,
            max_priority_fee_per_gas: 1_000,
            max_fee_per_gas: 25_000,
            gas_limit: 5_000_000,
            to: FRI_PRECOMPILE,
            value: Default::default(),
            input: stmt_hash.0.to_vec().into(),
            access_list: AccessList::default(),
            statement_versioned_hashes: vec![stmt_hash, stmt_hash],
        };
        let signed = unsigned.sign(wallet);
        let fri_tx = ZKsyncTxEnvelope::FriProof(signed);

        let output = tester.execute_block(vec![fri_tx]);

        // Tx succeeds and the precompile returns true for the
        // (single) verified hash.
        assert!(
            tx_succeeded(&output, 0),
            "duplicate-hash FRI_PROOF_TX must succeed, got: {:?}",
            output.tx_results[0]
        );

        // Two forward passes × one unique hash = 2 lookups.
        // Without dedup this would be 4 (2 passes × 2 hashes).
        let lookups = sidecar_lookups.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            lookups, 2,
            "expected exactly one sidecar lookup per forward pass \
             (unique hashes × passes = 1 × 2 = 2), got {}; the \
             validator must dedup duplicate statement hashes instead \
             of re-verifying them",
            lookups
        );
    }

    /// Pins that the FRI statement intrinsic gas surcharge is enforced during
    /// validation: a `FriProofTx` whose gas limit is too low to cover the
    /// per-statement FRI gas surcharge must be rejected before the verifier runs.
    ///
    /// Numeric setup (rig defaults):
    ///   `native_price = 10`, `base_fee = 1000`.
    #[test]
    fn fri_proof_tx_insufficient_gas_for_verifier_is_rejected() {
        use rig::zksync_os_interface::error::InvalidTransaction;

        let (setup_path, layout_path) = default_setup_and_layout_paths();
        let Some((proof_bytes, stmt_hash)) =
            load_proof_fixture(FRI_PRIMARY_PROOF_FIXTURE, &setup_path, &layout_path)
        else {
            return;
        };
        let artifacts = load_verifier_artifacts(&setup_path, &layout_path)
            .expect("artifacts must be present if fixture is");

        let stmt_bytes32 = rig::zk_ee::utils::Bytes32::from_array(stmt_hash.0);
        let (mut tester, _counter) = TestingFramework::new()
            .with_mock_fri_sidecars([(stmt_bytes32, proof_bytes)], Some(artifacts));
        let wallet = tester.prefunded_random_signer();

        let unsigned = UnsignedZKsyncFriProofTx {
            chain_id: 37,
            nonce: 0,
            max_priority_fee_per_gas: 9_000,
            max_fee_per_gas: 10_000,
            gas_limit: 30_000,
            to: FRI_PRECOMPILE,
            value: Default::default(),
            input: stmt_hash.0.to_vec().into(),
            access_list: AccessList::default(),
            statement_versioned_hashes: vec![stmt_hash],
        };
        let signed = unsigned.sign(wallet);
        let fri_tx = ZKsyncTxEnvelope::FriProof(signed);

        let output = tester.execute_block(vec![fri_tx]);

        assert!(
            matches!(
                output.tx_results[0],
                Err(InvalidTransaction::OutOfGasDuringValidation)
            ),
            "tx whose gas budget cannot cover the FRI intrinsic gas \
             charge must be rejected during validation; got: {:?}",
            output.tx_results[0]
        );
    }

    // NOTE: see the NOTE earlier in this module for why
    // `fri_proof_tx_rejects_statement_hash_mismatch` and
    // `fri_proof_tx_rejects_corrupted_proof_bytes` were removed.

    /// Pins the per-tx cap on `statement_versioned_hashes`:
    /// `MAX_FRI_STATEMENTS_PER_TX = 8`. A 9-hash list must be rejected
    /// at validation.
    ///
    /// Observable maps to `InvalidStructure` via the forward-system
    /// converter; we assert on that and on "tx dropped" — the specific
    /// `TooManyFriStatements` variant is only visible in bootloader
    /// traces.
    #[test]
    fn fri_proof_tx_rejects_list_over_cap() {
        use rig::zksync_os_interface::error::InvalidTransaction;

        // 9 distinct hashes, one over the cap of 8.
        let hashes: Vec<B256> = (0..9u8).map(|i| B256::from([i; 32])).collect();

        // Empty sidecar — the cap check runs before the verifier, so
        // we don't need proof bytes for the first eight entries.
        let (mut tester, _counter) = TestingFramework::new().with_mock_fri_sidecars(
            std::iter::empty::<(rig::zk_ee::utils::Bytes32, Vec<u8>)>(),
            None,
        );
        let wallet = tester.prefunded_random_signer();

        // Gas budget must cover the per-statement FRI intrinsic gas surcharge and enough
        // native for the signed hashes, so we actually land on the cap check
        // inside `build_verified_fri_statements_list` rather than being bounced
        // earlier by gas/native resource validation.
        let unsigned = UnsignedZKsyncFriProofTx {
            chain_id: 37,
            nonce: 0,
            max_priority_fee_per_gas: 1_000,
            max_fee_per_gas: 25_000,
            gas_limit: 5_000_000,
            to: FRI_PRECOMPILE,
            value: Default::default(),
            input: hashes[0].0.to_vec().into(),
            access_list: AccessList::default(),
            statement_versioned_hashes: hashes,
        };
        let signed = unsigned.sign(wallet);
        let fri_tx = ZKsyncTxEnvelope::FriProof(signed);

        let output = tester.execute_block(vec![fri_tx]);

        assert!(
            matches!(
                output.tx_results[0],
                Err(InvalidTransaction::TooManyFriStatements)
            ),
            "tx with 9 statement hashes (cap=8) must be rejected; \
             got: {:?}",
            output.tx_results[0]
        );
    }
}

// ---------------------------------------------------------------------------
// `forward_system::run::validate_fri_statement` integration tests.
//
// These cover the public admission API the sequencer calls before
// admitting a `FriProofTx` to the mempool. Each test uses the same
// gzipped fixture proof + setup/layout fixtures as `fri_precompile_e2e`
// (so we don't regenerate proofs) but bypasses the bootloader entirely:
// the admission entry point composes decode-and-flatten + host
// verifier directly, and we assert it returns the right
// `FriAdmissionError` for both valid input and several adversarial
// shapes.
//
// All tests silently no-op when the fixture is absent so CI without
// large-binary access still passes — same pattern as the existing
// e2e test.
// ---------------------------------------------------------------------------
#[cfg(feature = "fri_precompile")]
mod fri_admission_api {
    use super::fri_precompile_e2e::{
        default_setup_and_layout_paths, load_proof_fixture, load_verifier_artifacts,
        maybe_decompress, resolve_path, FRI_PRIMARY_PROOF_FIXTURE,
    };
    use rig::alloy::primitives::B256;
    use rig::forward_system::run::{validate_fri_statement, FriAdmissionError, FriHostVerifyError};
    use rig::zk_ee::utils::Bytes32;

    fn b256_to_bytes32(hash: B256) -> Bytes32 {
        Bytes32::from_array(hash.0)
    }

    /// Real fixture proof + correct statement hash → admitted.
    #[test]
    fn admits_valid_proof_with_correct_hash() {
        let (setup_path, layout_path) = default_setup_and_layout_paths();
        let Some((proof_bytes, stmt_hash)) =
            load_proof_fixture(FRI_PRIMARY_PROOF_FIXTURE, &setup_path, &layout_path)
        else {
            return;
        };
        let Some(artifacts) = load_verifier_artifacts(&setup_path, &layout_path) else {
            // Fixture present but artifacts absent — that's a misconfigured
            // checkout, not a missing-fixture skip. Don't silently pass.
            panic!("proof fixture loaded but verifier artifacts missing");
        };

        validate_fri_statement(b256_to_bytes32(stmt_hash), &proof_bytes, &artifacts)
            .expect("real fixture proof must admit against its derived statement hash");
    }

    /// Real fixture proof + wrong hash → `StatementHashMismatch`.
    ///
    /// This is the load-bearing rejection: a proof can verify
    /// internally but prove a different statement than the gateway
    /// signed. Admission must catch that.
    #[test]
    fn rejects_mismatched_statement_hash() {
        let (setup_path, layout_path) = default_setup_and_layout_paths();
        let Some((proof_bytes, _correct_hash)) =
            load_proof_fixture(FRI_PRIMARY_PROOF_FIXTURE, &setup_path, &layout_path)
        else {
            return;
        };
        let Some(artifacts) = load_verifier_artifacts(&setup_path, &layout_path) else {
            panic!("proof fixture loaded but verifier artifacts missing");
        };

        // All-zero hash is not the derived hash; the verifier will
        // succeed but its output won't keccak to zero.
        let bogus = Bytes32::ZERO;
        let err = validate_fri_statement(bogus, &proof_bytes, &artifacts)
            .expect_err("mismatched hash must be rejected");
        assert_eq!(
            err,
            FriAdmissionError::Verify(FriHostVerifyError::StatementHashMismatch),
            "wrong hash must surface as StatementHashMismatch, got {err:?}"
        );
    }

    /// Random bytes that aren't a bincode-encoded proof → decoder
    /// rejects before the verifier is touched.
    #[test]
    fn rejects_garbage_bytes() {
        // We don't need fixtures for this — the decode step gates
        // bytes regardless of artifacts shape. But we still need an
        // artifacts value to call the API, so skip when absent.
        let (setup_path, layout_path) = default_setup_and_layout_paths();
        let Some(artifacts) = load_verifier_artifacts(&setup_path, &layout_path) else {
            return;
        };

        let garbage: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        let err = validate_fri_statement(Bytes32::ZERO, &garbage, &artifacts)
            .expect_err("garbage bytes must be rejected");
        assert_eq!(
            err,
            FriAdmissionError::BincodeDecode,
            "garbage must surface as BincodeDecode, got {err:?}"
        );
    }

    /// Empty byte slice → `BincodeDecode`.
    #[test]
    fn rejects_empty_bytes() {
        let (setup_path, layout_path) = default_setup_and_layout_paths();
        let Some(artifacts) = load_verifier_artifacts(&setup_path, &layout_path) else {
            return;
        };

        let err = validate_fri_statement(Bytes32::ZERO, &[], &artifacts)
            .expect_err("empty bytes must be rejected");
        assert_eq!(
            err,
            FriAdmissionError::BincodeDecode,
            "empty input must surface as BincodeDecode, got {err:?}"
        );
    }

    /// Truncated valid proof — bincode-decodes part of the structure
    /// then runs out of bytes. Acceptable outcomes: `BincodeDecode`
    /// (truncation caught by the decoder) or `VerifierRejected`
    /// (partial structure decoded but the verifier rejects the
    /// flattened stream). Both are valid rejection paths; just not
    /// `Ok` and not `StatementHashMismatch`.
    #[test]
    fn rejects_truncated_proof() {
        let proof_path = resolve_path("FRI_ORACLE_PATH", FRI_PRIMARY_PROOF_FIXTURE);
        if !proof_path.exists() {
            return;
        }
        let fixture_bytes = std::fs::read(&proof_path).expect("read proof");
        let proof_bytes = maybe_decompress(&fixture_bytes);

        let (setup_path, layout_path) = default_setup_and_layout_paths();
        let Some(artifacts) = load_verifier_artifacts(&setup_path, &layout_path) else {
            return;
        };

        // Halve the bytes — almost certainly leaves the bincode
        // structure incomplete.
        let half = &proof_bytes[..proof_bytes.len() / 2];
        let err = validate_fri_statement(Bytes32::ZERO, half, &artifacts)
            .expect_err("truncated proof must be rejected");
        assert!(
            matches!(
                err,
                FriAdmissionError::BincodeDecode
                    | FriAdmissionError::Verify(
                        FriHostVerifyError::VerifierRejected
                            | FriHostVerifyError::TrailingWords
                            | FriHostVerifyError::UnsupportedOpType,
                    )
            ),
            "truncated proof must reject as BincodeDecode or verifier failure, got {err:?}"
        );
    }
}

/// Measures the gas cost of BALANCE on `target` by deploying a probe contract,
/// executing it, and reading the stored gas measurement from slot 0.
fn measure_balance_gas_cost(target: Address) -> u64 {
    measure_balance_gas_cost_inner(target, false)
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

/// EVM precompiles (including the Pectra additions: BLAKE2F 0x09, point eval
/// 0x0a, BLS12-381 0x0b..0x11, P256 0x100) must be warm at transaction start.
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

    // Pectra precompiles are always registered, so they must be warm. The AtlasV4
    // REVM spec warms them too, so the consistency check holds.
    let blake2f = address!("0000000000000000000000000000000000000009");
    assert_eq!(
        measure_balance_gas_cost(blake2f),
        WARM_BALANCE,
        "blake2f (0x09) must be warm"
    );

    let point_eval = address!("000000000000000000000000000000000000000a");
    assert_eq!(
        measure_balance_gas_cost(point_eval),
        WARM_BALANCE,
        "point_eval (0x0a) must be warm"
    );

    // BLS12-381 precompiles (0x0b..0x11, EIP-2537).
    let bls12_g1add = address!("000000000000000000000000000000000000000b");
    assert_eq!(
        measure_balance_gas_cost(bls12_g1add),
        WARM_BALANCE,
        "bls12_381 g1add (0x0b) must be warm"
    );
    let bls12_map_fp2_to_g2 = address!("0000000000000000000000000000000000000011");
    assert_eq!(
        measure_balance_gas_cost(bls12_map_fp2_to_g2),
        WARM_BALANCE,
        "bls12_381 map_fp2_to_g2 (0x11) must be warm"
    );

    // P256 verify (0x100, RIP-7212 / EIP-7951).
    let p256_verify = address!("0000000000000000000000000000000000000100");
    assert_eq!(
        measure_balance_gas_cost(p256_verify),
        WARM_BALANCE,
        "p256 verify (0x100) must be warm"
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
