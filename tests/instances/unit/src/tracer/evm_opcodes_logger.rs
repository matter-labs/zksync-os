#![cfg(test)]

//!
//! Test for the EvmOpcodesLogger tracer.
//!
//! This test verifies that EvmOpcodesLogger correctly captures EVM execution steps,
//! gas usage, storage operations, and call frame management.

use rig::alloy::consensus::TxEip2930;
use rig::alloy::primitives::{address, Address, TxKind, U256};
use rig::forward_system::system::system::ForwardRunningSystem;
use rig::forward_system::system::tracers::evm_opcodes_logger::EvmOpcodesLogger;
use rig::ruint::aliases::B160;
use rig::Chain;

fn run_chain_with_tracer(
    to: Address,
    contracts: Vec<(Address, Vec<u8>)>,
    tracer: &mut EvmOpcodesLogger<ForwardRunningSystem>,
) {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();

    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );

    for (address, bytecode) in contracts {
        chain.set_evm_bytecode(B160::from_be_bytes(address.into_array()), &bytecode);
    }

    // Create transaction to call the contract
    let encoded_tx = {
        let tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 100_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let result = chain.run_block_with_extra_stats(vec![encoded_tx], None, None, tracer);

    assert!(result.is_ok(), "Block execution should succeed");
    let (block_output, _, _) = result.unwrap();
    assert!(
        block_output.tx_results[0].is_ok(),
        "Transaction should succeed. Result: {:?}",
        block_output.tx_results[0]
    );
}

fn check_opcodes(mut opcodes_iter: std::slice::Iter<'_, &String>, expected_opcodes: Vec<&str>) {
    for opcode in expected_opcodes {
        assert_eq!(opcodes_iter.next().unwrap().as_str(), opcode);
    }
}

#[test]
fn test_evm_opcodes_logger_basic_execution() {
    let contract_address = address!("1000000000000000000000000000000000000001");

    // Simple contract bytecode:
    // PUSH1 0x42    -> 6042
    // PUSH1 0x00    -> 6000
    // MSTORE        -> 52     (store 0x42 at memory position 0)
    // PUSH1 0x20    -> 6020
    // PUSH1 0x00    -> 6000
    // RETURN        -> f3     (return 32 bytes from memory position 0)
    let test_contract_bytecode = hex::decode("604260005260206000f3").unwrap();

    let mut tracer = EvmOpcodesLogger::default();
    run_chain_with_tracer(
        contract_address,
        vec![(contract_address, test_contract_bytecode)],
        &mut tracer,
    );

    // Verify transaction log was created
    assert_eq!(
        tracer.transaction_logs.len(),
        1,
        "Should have one transaction log"
    );

    let tx_log = &tracer.transaction_logs[0];
    assert!(
        tx_log.finished,
        "Transaction log should be marked as finished"
    );
    assert!(
        !tx_log.steps.is_empty(),
        "Should have captured execution steps"
    );

    // Check that we captured the expected opcodes
    let opcodes: Vec<&String> = tx_log
        .steps
        .iter()
        .filter_map(|step| step.opcode.as_ref())
        .collect();

    // Should contain PUSH1, MSTORE, RETURN opcodes
    check_opcodes(
        opcodes.iter(),
        vec!["PUSH1", "PUSH1", "MSTORE", "PUSH1", "PUSH1", "RETURN"],
    );

    // Verify call depth tracking
    for step in &tx_log.steps {
        assert_eq!(
            step.depth, 1,
            "All steps should be at depth 1 (called contract)"
        );
    }
}

#[test]
fn test_evm_opcodes_logger_with_storage() {
    let contract_address = address!("1000000000000000000000000000000000000001");

    // Contract bytecode that uses storage:
    // PUSH1 0x42    -> 6042  (value to store)
    // PUSH1 0x00    -> 6000  (storage slot)
    // SSTORE        -> 55    (store value in slot)
    // PUSH1 0x00    -> 6000  (storage slot)
    // SLOAD         -> 54    (load value from slot)
    // PUSH1 0x00    -> 6000  (memory position)
    // MSTORE        -> 52    (store loaded value in memory)
    // PUSH1 0x20    -> 6020  (return size)
    // PUSH1 0x00    -> 6000  (memory position)
    // RETURN        -> f3    (return)
    let test_contract_bytecode = hex::decode("604260005560005460005260206000f3").unwrap();

    // Create tracer with storage tracking enabled
    let mut tracer = EvmOpcodesLogger::new_with_config(
        false, // enable_memory
        true,  // enable_stack
        false, // enable_returndata
        true,  // enable_storage
        false, // enable_transient_storage
        0,     // no limit
    );
    run_chain_with_tracer(
        contract_address,
        vec![(contract_address, test_contract_bytecode)],
        &mut tracer,
    );

    let tx_log = &tracer.transaction_logs[0];
    assert!(
        tx_log.finished,
        "Transaction log should be marked as finished"
    );

    let opcodes: Vec<&String> = tx_log
        .steps
        .iter()
        .filter_map(|step| step.opcode.as_ref())
        .collect();

    check_opcodes(
        opcodes.iter(),
        vec![
            "PUSH1", "PUSH1", "SSTORE", "PUSH1", "SLOAD", "PUSH1", "MSTORE", "PUSH1", "PUSH1",
            "RETURN",
        ],
    );

    // Verify storage information is captured in steps
    let storage_steps: Vec<_> = tx_log
        .steps
        .iter()
        .filter(|step| step.storage.is_some())
        .collect();

    assert!(
        !storage_steps.is_empty(),
        "Should have steps with storage information"
    );
}

#[test]
fn test_evm_opcodes_logger_with_limit() {
    let contract_address = address!("1000000000000000000000000000000000000001");

    // Contract with many operations to test limit
    // Multiple PUSH operations followed by POP operations
    let test_contract_bytecode =
        hex::decode("6001600260036004600560066007600850505050505050").unwrap();

    // Create tracer with step limit
    let mut tracer = EvmOpcodesLogger::new_with_config(
        false, // enable_memory
        true,  // enable_stack
        false, // enable_returndata
        false, // enable_storage
        false, // enable_transient_storage
        5,     // limit to 5 steps
    );

    run_chain_with_tracer(
        contract_address,
        vec![(contract_address, test_contract_bytecode)],
        &mut tracer,
    );

    let tx_log = &tracer.transaction_logs[0];
    assert!(
        tx_log.finished,
        "Transaction log should be marked as finished"
    );

    assert_eq!(tx_log.steps.len(), 5);
}

#[test]
fn test_evm_opcodes_logger_memory_and_stack_capture() {
    let contract_address = address!("1000000000000000000000000000000000000001");

    // Simple contract that manipulates memory and stack
    let test_contract_bytecode = hex::decode("604260005260206000f3").unwrap();

    // Create tracer with memory and stack capture enabled
    let mut tracer = EvmOpcodesLogger::new_with_config(
        true,  // enable_memory
        true,  // enable_stack
        true,  // enable_returndata
        false, // enable_storage
        false, // enable_transient_storage
        0,     // no limit
    );

    run_chain_with_tracer(
        contract_address,
        vec![(contract_address, test_contract_bytecode)],
        &mut tracer,
    );

    let tx_log = &tracer.transaction_logs[0];
    assert!(
        tx_log.finished,
        "Transaction log should be marked as finished"
    );

    // Verify that memory and stack information is captured
    let steps_with_memory: Vec<_> = tx_log
        .steps
        .iter()
        .filter(|step| step.memory.is_some())
        .collect();

    let steps_with_stack: Vec<_> = tx_log
        .steps
        .iter()
        .filter(|step| step.stack.is_some())
        .collect();

    // All steps should have stack info when enabled
    assert_eq!(
        steps_with_stack.len(),
        tx_log.steps.len(),
        "All steps should have stack information when enabled"
    );

    // All steps should have memory info when enabled
    assert_eq!(
        steps_with_memory.len(),
        tx_log.steps.len(),
        "All steps should have memory information when enabled"
    );
}
