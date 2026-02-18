//!
//! These tests are focused on system hooks functionality.
//!
#![cfg(test)]

use alloy_sol_types::{sol, SolEvent};
use rig::alloy::primitives::address;
use rig::alloy::primitives::Address;
use rig::forward_system::run::convert_alloy::FromAlloy;
use rig::ruint::aliases::B160;
use rig::ruint::aliases::U256;
use rig::system_hooks::addresses_constants::L2_INTEROP_ROOT_STORAGE_ADDRESS;
use rig::system_hooks::addresses_constants::SYSTEM_CONTEXT_ADDRESS;
use rig::testing_utils::call_address_and_measure_gas_cost;
use rig::testing_utils::install_system_contracts;
use rig::utils::{
    address_into_special_storage_key, AccountProperties, L1TxBuilder,
    ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use rig::zk_ee::utils::Bytes32;
use rig::zksync_os_interface::types::{BlockOutput, ExecutionResult};
use rig::{alloy, Chain};
use zksync_os_tests_common::zksync_tx::encoding::ZKsyncOsEncodable;

fn bal(chain: &mut Chain, addr: alloy::primitives::Address) -> alloy::primitives::U256 {
    chain
        .get_account_properties(&B160::from_be_bytes(addr.into_array()))
        .balance
}

fn tx_succeeded(output: &BlockOutput, idx: usize) -> bool {
    output.tx_results[idx]
        .as_ref()
        .ok()
        .map(|o| o.is_success())
        .unwrap_or(false)
}

fn tx_failed(output: &BlockOutput, idx: usize) -> bool {
    !tx_succeeded(output, idx)
}

#[test]
fn test_value_transfer_fails_if_insufficient_balance_max_msg_value() {
    let mut chain = Chain::empty(None);

    let sender = address!("1234567890123456789012345678901234567890");
    let recipient = address!("2222567890123456789012345678901234567890");

    // Sender has 1 wei, tries to send 2^256 wei.
    let initial_sender = alloy::primitives::U256::from(1u64);
    let value = alloy::primitives::U256::MAX;

    chain.set_balance(B160::from_be_bytes(sender.into_array()), initial_sender);

    let encoded = L1TxBuilder::new()
        .from(sender)
        .to(recipient)
        .input(Vec::new())
        .value(value)
        // keep fees at 0 so we can assert balances are unchanged on failure
        .gas_price(0)
        .gas_limit(200_000)
        .nonce(0)
        .build()
        .encode();
    let output = chain.run_block(vec![encoded], None, None, None);

    assert!(
        tx_failed(&output, 0),
        "tx must fail when msg.value > sender balance"
    );

    // Balances must be unchanged (no fees).
    assert_eq!(
        bal(&mut chain, sender),
        initial_sender,
        "sender balance must not change"
    );
    assert_eq!(
        bal(&mut chain, recipient),
        alloy::primitives::U256::ZERO,
        "recipient must not receive value"
    );
}

#[test]
fn test_l2_base_token_withdraw_fails_if_insufficient_balance() {
    let mut chain = Chain::empty(None);

    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");

    // Sender has 1 wei, tries to withdraw 2 eth.
    let initial_sender = alloy::primitives::U256::from(1u64);
    let value = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH

    chain.set_balance(B160::from_be_bytes(sender.into_array()), initial_sender);

    // withdraw(address) selector 0x51cff8d9
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&hex::decode("51cff8d9").unwrap());
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(l1_receiver.as_slice());

    let encoded = L1TxBuilder::new()
        .from(sender)
        .to(l2_base_token_address)
        .input(calldata)
        .value(value)
        .gas_price(0)
        .gas_limit(200_000)
        .nonce(0)
        .build()
        .encode();
    let output = chain.run_block(vec![encoded], None, None, None);

    assert!(
        tx_failed(&output, 0),
        "withdraw must fail when msg.value > sender balance"
    );

    // Balances unchanged
    assert_eq!(
        bal(&mut chain, sender),
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
    let mut chain = Chain::empty(None);

    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let additional_data = b"test message data";

    // Sender has 1 wei, tries to withdrawWithMessage 2 eth.
    let initial_sender = alloy::primitives::U256::from(1u64);
    let value = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH

    chain.set_balance(B160::from_be_bytes(sender.into_array()), initial_sender);

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

    let encoded = L1TxBuilder::new()
        .from(sender)
        .to(l2_base_token_address)
        .input(calldata)
        .value(value)
        .gas_price(0)
        .gas_limit(300_000)
        .nonce(0)
        .build()
        .encode();
    let output = chain.run_block(vec![encoded], None, None, None);

    assert!(
        tx_failed(&output, 0),
        "withdrawWithMessage must fail when msg.value > sender balance"
    );

    // Balances unchanged
    assert_eq!(
        bal(&mut chain, sender),
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
    let mut chain = Chain::empty(None);

    let sender = address!("1234567890123456789012345678901234567890");
    let recipient = address!("2222567890123456789012345678901234567890");
    let value = alloy::primitives::U256::from(1_000_000_000_000_000_000u64); // 1 ETH

    // Fund sender so `msg.value` can be paid from L2 balance.
    chain.set_balance(B160::from_be_bytes(sender.into_array()), value);

    let encoded = L1TxBuilder::new()
        .from(sender)
        .to(recipient)
        .input(Vec::new())
        .value(value)
        // keep fees minimal to reduce side-effects
        .gas_price(0)
        .gas_limit(200_000)
        .nonce(0)
        .build()
        .encode();
    let output = chain.run_block(vec![encoded], None, None, None);

    assert!(
        tx_succeeded(&output, 0),
        "tx must succeed with sufficient L2 balance"
    );
    assert_eq!(
        bal(&mut chain, recipient),
        value,
        "recipient must receive msg.value"
    );
}

#[test]
fn test_set_bytecode_details_evm() {
    let mut chain = Chain::empty(None);

    let contract_deployer_address = address!("0000000000000000000000000000000000008006");
    let contract_deployer_hook_address = address!("0000000000000000000000000000000000007002");

    let bytecode = hex::decode("0123456789").unwrap();
    let code_hash = Bytes32::from_array(
        hex::decode("1c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    chain.set_preimage(code_hash, &bytecode);
    let calldata =
        hex::decode("00000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b944")
            .unwrap();

    chain.set_balance(
        B160::from_alloy(contract_deployer_address),
        U256::from(1_000_000_000_000_000_u64),
    );

    let encoded_tx = {
        let tx = L1TxBuilder::new()
            .from(contract_deployer_address)
            .to(contract_deployer_hook_address)
            .input(calldata)
            .gas_price(1000)
            .gas_limit(200_000)
            .build();
        tx.encode()
    };
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

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
    install_system_contracts(&mut chain, false, false, true);

    let bytecode = hex::decode("0123456789").unwrap();
    let code_hash = Bytes32::from_array(
        hex::decode("1c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    chain.set_preimage(code_hash, &bytecode);

    let from = address!("000000000000000000000000000000000000800e");
    let contract_deployer_address = address!("0000000000000000000000000000000000008006");
    let calldata =
        hex::decode("231b395700000000000000000000000000000000000000000000000000000000000100021c4be3dec3ba88b69a8d3cd5cedd2b22f3da89b1ff9c8fd453c5a6e10c23d6f7000000000000000000000000000000000000000000000000000000000000000579fad56e6cf52d0c8c2c033d568fc36856ba2b556774960968d79274b0e6b9440000000000000000000000000000000000000000000000000000000000000005")
            .unwrap();

    let encoded_tx = {
        let tx = L1TxBuilder::new()
            .from(from)
            .to(contract_deployer_address)
            .input(calldata)
            .gas_price(1000)
            .gas_limit(200_000)
            .build();
        tx.encode()
    };
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

    let result = &output
        .tx_results
        .first()
        .unwrap()
        .as_ref()
        .unwrap()
        .execution_result;

    assert!(matches!(result, ExecutionResult::Revert(_)));
}

#[test]
fn test_l1_messenger_hook_succeeds() {
    let mut chain = Chain::empty(None);
    // making sure hooks are installed
    install_system_contracts(&mut chain, false, false, true);

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

    let encoded_tx = tx.encode();
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

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
    let mut chain = Chain::empty(None);
    // making sure hooks are installed
    install_system_contracts(&mut chain, false, false, true);

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

    let encoded_tx = tx.encode();
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

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
    let mut chain = Chain::empty(None);
    // making sure hooks are installed
    install_system_contracts(&mut chain, false, false, true);

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
        .input(hook_calldata)
        .gas_price(1000)
        .gas_limit(200_000)
        .build();

    let encoded_tx = tx.encode();
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

    let tx_result = &output
        .tx_results
        .first()
        .unwrap()
        .as_ref()
        .unwrap()
        .execution_result;

    assert!(matches!(tx_result, ExecutionResult::Success { .. }));
}

#[test]
fn test_l2_base_token_withdraw_events() {
    let mut chain = Chain::empty(None);
    install_system_contracts(&mut chain, true, true, false);

    // L2 base token address is 0x800a
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(1000000000000000000u64); // 1 ETH

    chain.set_balance(B160::from_alloy(sender), withdrawal_amount);

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

    let encoded_tx = tx.encode();
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

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
    let mut chain = Chain::empty(None);
    install_system_contracts(&mut chain, true, true, false);

    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH
    let additional_data = b"test message data";

    // Set up initial balance for the sender
    chain.set_balance(B160::from_alloy(sender), withdrawal_amount);

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

    let encoded_tx = tx.encode();
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

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
    let mut chain = Chain::empty(None);
    install_system_contracts(&mut chain, true, true, false);

    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(1000000000000000000u64); // 1 ETH

    // Deliberately set invalid balance (insufficient funds)
    // Set up initial balance for the sender
    chain.set_balance(B160::from_alloy(sender), withdrawal_amount);

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

    let encoded_tx = tx.encode();
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

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
    install_system_contracts(&mut chain, true, true, false);

    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let l1_receiver = address!("0987654321098765432109876543210987654321");
    let withdrawal_amount = alloy::primitives::U256::from(2000000000000000000u64); // 2 ETH
    let additional_data = b"test message data";

    // Set up initial balance for the sender
    chain.set_balance(B160::from_alloy(sender), withdrawal_amount);

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

    let encoded_tx = tx.encode();
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

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
    let mut chain = Chain::empty(None);

    // L2 base token address is 0x800a
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    let sender = address!("1234567890123456789012345678901234567890");
    let recipient = address!("2222567890123456789012345678901234567890");
    let mint_amount = alloy::primitives::U256::from(5000000000000000000u64); // 5 ETH

    chain.set_balance(B160::from_be_bytes(sender.into_array()), mint_amount);

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

    let encoded_tx = tx.encode();
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

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

    // The hook should charge HOOK_BASE_ERGS_COST (100 gas) + extra for bytecode length
    assert_eq!(gas_used, 2950);
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
    let mut chain = Chain::empty(None);

    chain.mint_tokens_to_treasury(); // to properly reflect the initial balance

    // L2 base token address is the only address allowed to call the mint hook
    let l2_base_token_address = address!("000000000000000000000000000000000000800a");
    // Mint hook address (0x7100)
    let mint_hook_address = address!("0000000000000000000000000000000000007100");
    let mint_amount = alloy::primitives::U256::from(3000000000000000000u64); // 3 ETH

    // Check initial balance of L2_BASE_TOKEN_ADDRESS is zero
    let initial_balance = chain
        .get_account_properties(&B160::from_alloy(l2_base_token_address))
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

    let encoded_tx = tx.encode();
    let transactions = vec![encoded_tx];

    let output = chain.run_block(transactions, None, None, None);

    // Assert transaction succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {} failed with: {:?}", i, r)
        }
        success
    }));

    // Check that the caller's (L2_BASE_TOKEN_ADDRESS) balance was increased by the mint amount
    let final_balance = chain
        .get_account_properties(&B160::from_alloy(l2_base_token_address))
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
fn test_event_hooks_empty_topics() {
    for test_contract_address in [L2_INTEROP_ROOT_STORAGE_ADDRESS, SYSTEM_CONTEXT_ADDRESS] {
        let mut chain = Chain::empty(None);

        // Contract that emits a log with empty topics array - this should be handled gracefully
        // Before the fix, this would cause a panic due to missing bounds check
        let test_contract = Address::from(test_contract_address.to_be_bytes());

        // Bytecode that emits LOG0 (no topics)
        // PUSH1 0x00    -> 6000  (data offset)
        // PUSH1 0x00    -> 6000  (data length)
        // LOG0          -> a0    (emit log with no topics)
        // STOP          -> 00
        let test_contract_bytecode = hex::decode("60006000a000").unwrap();

        let tx = L1TxBuilder::new()
            .from(address!("1234567890123456789012345678901234567890"))
            .to(test_contract)
            .input(hex::decode("").unwrap())
            .gas_price(1000)
            .gas_limit(200_000)
            .build();

        chain.set_evm_bytecode(
            B160::from_be_bytes(test_contract.clone().into_array()),
            &test_contract_bytecode,
        );

        let encoded_tx = tx.encode();
        let transactions = vec![encoded_tx];

        let output = chain.run_block(transactions, None, None, None);

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
