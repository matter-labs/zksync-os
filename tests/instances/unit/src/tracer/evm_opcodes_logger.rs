#![cfg(test)]

//!
//! Test for the EvmOpcodesLogger tracer.
//!
//! This is a minimalistic sanity checking. Does not properly cover all cases and functionality

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

    let result = chain.run_block_with_extra_stats(vec![encoded_tx], None, None, None, tracer);

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

const LOW: u64 = 3;

#[test]
fn test_evm_opcodes_logger_simple_gas_cost() {
    let to_address = address!("1000000000000000000000000000000000000001");

    /*
    contract A {
        fallback() external payable {
            address(0x1000000000000000000000000000000000000002).call("");
        }
    }
    */
    let test_contract_bytecode = hex::decode("608060405273100000000000000000000000000000000000000273ffffffffffffffffffffffffffffffffffffffff1660405160399060a0565b5f604051808303815f865af19150503d805f81146070576040519150601f19603f3d011682016040523d82523d5f602084013e6075565b606091505b005b5f81905092915050565b50565b5f608d5f836077565b91506096826081565b5f82019050919050565b5f60a8826084565b915081905091905056fea2646970667358221220a15e432da1806529f340873fa872e90fa96960e21ae9cf47afd4fb55cd01fb2064736f6c634300081e0033").unwrap();

    let contract2_address = address!("1000000000000000000000000000000000000002");
    // Simple contract that manipulates memory and stack
    let contract2_bytecode = hex::decode("604260005260206000f3").unwrap();

    let mut tracer = EvmOpcodesLogger::default();
    run_chain_with_tracer(
        to_address,
        vec![
            (to_address, test_contract_bytecode),
            (contract2_address, contract2_bytecode),
        ],
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

    for step in tx_log.steps.iter() {
        let opcode = step.opcode.as_ref().unwrap();
        let expected_gas = match opcode.as_str() {
            "PUSH0" => 2,
            "PUSH1" => LOW,
            "PUSH20" => LOW,
            "AND" => LOW,
            "SWAP1" => LOW,
            "SWAP2" => LOW,
            "SWAP3" => LOW,
            "JUMP" => 8,
            "JUMPDEST" => 1,
            "DUP2" => LOW,
            "DUP3" => LOW,
            "DUP4" => LOW,
            "DUP5" => LOW,
            "POP" => 2,
            "ADD" => LOW,
            "SUB" => LOW,
            "GAS" => 2,
            "CALL" => 2600, // cold access expected
            "RETURN" => 0,
            "RETURNDATASIZE" => 2,
            "RETURNDATACOPY" => 9, // expected in this case
            "EQ" => LOW,
            "STOP" => 0,
            _ => {
                continue; // skip
            }
        };
        assert_eq!(
            step.gas_used,
            Some(expected_gas),
            "Invalid gas for {:?}: {:?}",
            step.opcode,
            step.gas_used
        );
    }
}

#[test]
fn test_evm_opcodes_logger_gas_cost_call_corner_case_2() {
    let to_address = address!("1000000000000000000000000000000000000001");

    // minimalistic call as a last opcode
    let test_contract_bytecode =
        hex::decode("600060006000600060007310000000000000000000000000000000000000025af1").unwrap();

    let mut tracer = EvmOpcodesLogger::default();
    run_chain_with_tracer(
        to_address,
        vec![(to_address, test_contract_bytecode)],
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

    for step in tx_log.steps.iter() {
        let opcode = step.opcode.as_ref().unwrap();
        let expected_gas = match opcode.as_str() {
            "PUSH0" => 2,
            "PUSH1" => LOW,
            "PUSH20" => LOW,
            "AND" => LOW,
            "SWAP1" => LOW,
            "SWAP2" => LOW,
            "SWAP3" => LOW,
            "JUMP" => 8,
            "JUMPDEST" => 1,
            "DUP2" => LOW,
            "DUP3" => LOW,
            "DUP4" => LOW,
            "DUP5" => LOW,
            "POP" => 2,
            "ADD" => LOW,
            "SUB" => LOW,
            "GAS" => 2,
            "CALL" => 2600, // cold access expected
            "RETURN" => 0,
            "RETURNDATASIZE" => 2,
            "RETURNDATACOPY" => 9, // expected in this case
            "EQ" => LOW,
            "STOP" => 0,
            _ => {
                continue; // skip
            }
        };
        assert_eq!(
            step.gas_used,
            Some(expected_gas),
            "Invalid gas for {:?}: {:?}",
            step.opcode,
            step.gas_used
        );
    }
}
