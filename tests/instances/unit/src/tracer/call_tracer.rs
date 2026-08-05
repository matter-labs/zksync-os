#![cfg(test)]

//!
//! Tests for the CallTracer tracer.
//!
//! This is a minimalistic sanity checking. Does not properly cover all cases and functionality

use rig::alloy::primitives::address;
use rig::forward_system::system::tracers::call_tracer::{CallTracer, CallType};
use rig::ruint::aliases::B160;

use crate::tracer::run_chain_with_tracer;

#[test]
fn test_call_tracer_basic_call() {
    let contract_address = address!("1000000000000000000000000000000000000001");

    // Simple contract bytecode that returns a value:
    // PUSH1 0x42    -> 6042
    // PUSH1 0x00    -> 6000
    // MSTORE        -> 52     (store 0x42 at memory position 0)
    // PUSH1 0x20    -> 6020
    // PUSH1 0x00    -> 6000
    // RETURN        -> f3     (return 32 bytes from memory position 0)
    let test_contract_bytecode = hex::decode("604260005260206000f3").unwrap();

    let mut tracer = CallTracer::default();
    run_chain_with_tracer(
        contract_address,
        vec![(contract_address, test_contract_bytecode)],
        &mut tracer,
    );

    // Verify transaction was captured
    assert_eq!(
        tracer.transactions.len(),
        1,
        "Should have one transaction recorded"
    );

    let call = &tracer.transactions[0];

    // Verify basic call properties
    assert!(matches!(call.call_type, CallType::Call));
    assert_eq!(call.to, B160::from_be_bytes(contract_address.into_array()));
    assert!(!call.reverted, "Call should not be reverted");
    assert!(call.error.is_none(), "Call should not have error");
    assert_eq!(call.gas, 100_000 - 21_000, "Call should have gas assigned");
    assert!(call.gas_used > 0, "Call should have used some gas");
    assert!(
        call.gas_used <= call.gas,
        "Gas used should not exceed gas limit"
    );
    assert!(call.calls.is_empty(), "Simple call should have no subcalls");
}

#[test]
fn test_call_tracer_nested_calls() {
    let contract_a_address = address!("1000000000000000000000000000000000000001");
    let contract_b_address = address!("1000000000000000000000000000000000000002");

    // Contract A calls Contract B
    let contract_a_bytecode =
        hex::decode("600060006000600060007310000000000000000000000000000000000000025af1").unwrap();

    // Contract B - simple return
    let contract_b_bytecode = hex::decode("604260005260206000f3").unwrap();

    let mut tracer = CallTracer::default();
    run_chain_with_tracer(
        contract_a_address,
        vec![
            (contract_a_address, contract_a_bytecode),
            (contract_b_address, contract_b_bytecode),
        ],
        &mut tracer,
    );

    // Verify transaction was captured
    assert_eq!(
        tracer.transactions.len(),
        1,
        "Should have one transaction recorded"
    );

    let main_call = &tracer.transactions[0];

    // Verify main call has subcalls
    assert_eq!(main_call.calls.len(), 1, "Should have one subcall");

    let subcall = &main_call.calls[0];
    assert!(matches!(subcall.call_type, CallType::Call));
    assert_eq!(
        subcall.to,
        B160::from_be_bytes(contract_b_address.into_array())
    );
    assert!(!subcall.reverted, "Subcall should not be reverted");
    assert!(subcall.error.is_none(), "Subcall should not have error");
    assert!(subcall.gas > 0, "Subcall should have gas assigned");
    assert!(subcall.gas_used > 0, "Subcall should have used some gas");
    assert!(
        subcall.gas_used <= subcall.gas,
        "Gas used should not exceed gas limit"
    );
}

#[test]
fn test_call_tracer_with_logs() {
    let contract_address = address!("1000000000000000000000000000000000000001");

    // Contract that emits a log (simplified LOG0 example)
    // PUSH1 0x00    -> 6000  (data offset)
    // PUSH1 0x00    -> 6000  (data length)
    // LOG0          -> a0    (emit log with no topics)
    // STOP          -> 00
    let test_contract_bytecode = hex::decode("60006000a000").unwrap();

    let mut tracer = CallTracer::new_with_config(true, false); // collect_logs = true
    run_chain_with_tracer(
        contract_address,
        vec![(contract_address, test_contract_bytecode)],
        &mut tracer,
    );

    let call = &tracer.transactions[0];

    assert_eq!(call.logs.len(), 1);
}

#[test]
fn test_call_tracer_only_top_call() {
    let contract_a_address = address!("1000000000000000000000000000000000000001");
    let contract_b_address = address!("1000000000000000000000000000000000000002");

    // Contract A calls Contract B
    let contract_a_bytecode =
        hex::decode("600060006000600060007310000000000000000000000000000000000000025af1").unwrap();

    // Contract B - simple return
    let contract_b_bytecode = hex::decode("604260005260206000f3").unwrap();

    let mut tracer = CallTracer::new_with_config(false, true); // only_top_call = true
    run_chain_with_tracer(
        contract_a_address,
        vec![
            (contract_a_address, contract_a_bytecode),
            (contract_b_address, contract_b_bytecode),
        ],
        &mut tracer,
    );

    let main_call = &tracer.transactions[0];

    assert_eq!(main_call.calls.len(), 0)
}

#[test]
fn test_call_tracer_return_data() {
    let contract_address = address!("1000000000000000000000000000000000000001");

    // PUSH32 with 32 bytes of data, then return it
    let test_contract_bytecode = hex::decode(
        "7f420000000000000000000000000000000000000000000000000000000000000060005260206000f3",
    )
    .unwrap();

    let mut tracer = CallTracer::default();
    run_chain_with_tracer(
        contract_address,
        vec![(contract_address, test_contract_bytecode)],
        &mut tracer,
    );

    let call = &tracer.transactions[0];

    // Verify output data is captured
    assert_eq!(
        call.output,
        hex::decode("4200000000000000000000000000000000000000000000000000000000000000").unwrap()
    );
}
