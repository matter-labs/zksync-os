//! EVM call execution outcomes: success, revert, invalid opcode, and out-of-gas.

use crate::test_support::{call_tx, call_tx_with, new_tester};
use rig::alloy::primitives::{address, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::constants::{
    CALL_GAS_LIMIT, DEFAULT_BALANCE, DEFAULT_MAX_FEE, DEFAULT_PRIORITY_FEE, TEST_CHAIN_ID,
};
use rig::evm_bytecode::{self, BytecodeBuilder};
use rig::ruint::aliases::U256;
use rig::{assert_tx_reverted, assert_tx_success};

#[test]
fn out_of_gas_mid_execution() {
    let loop_bytecode = evm_bytecode::infinite_loop();
    let contract = address!("0000000000000000000000000000000000000101");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &loop_bytecode);

    let tx = call_tx(signer, contract, 25_000);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn explicit_revert_no_data() {
    let revert_bytecode = evm_bytecode::revert();
    let contract = address!("0000000000000000000000000000000000000201");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &revert_bytecode);

    let tx = call_tx(signer, contract, CALL_GAS_LIMIT);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn explicit_revert_with_data() {
    let revert_with_data = evm_bytecode::revert_with_data(&[0xde, 0xad, 0xbe, 0xef]);
    let contract = address!("0000000000000000000000000000000000000202");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &revert_with_data);

    let tx = call_tx(signer, contract, CALL_GAS_LIMIT);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);

    let tx_out = output.tx_results[0].as_ref().unwrap();
    match &tx_out.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Revert(data) => {
            assert_eq!(data, &[0xde, 0xad, 0xbe, 0xef]);
        }
        _ => panic!("expected revert with payload"),
    }
}

#[test]
fn invalid_opcode() {
    let invalid_bytecode = evm_bytecode::invalid_opcode();
    let contract = address!("0000000000000000000000000000000000000203");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &invalid_bytecode);

    let tx = call_tx(signer, contract, CALL_GAS_LIMIT);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn call_to_eoa_with_calldata_succeeds() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let eoa = address!("0000000000000000000000000000000000000204");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = call_tx_with(
        signer,
        eoa,
        0,
        CALL_GAS_LIMIT,
        AlloyU256::ZERO,
        vec![0xca, 0xfe, 0xba, 0xbe],
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    );
    let output = tester.execute_block(vec![tx]);
    assert_tx_success!(output, 0);
}

#[test]
fn nested_call_inner_reverts_outer_succeeds() {
    let inner_revert = evm_bytecode::revert();
    let inner_addr = address!("0000000000000000000000000000000000000205");

    let outer_addr = address!("0000000000000000000000000000000000000206");
    let outer_bytecode = BytecodeBuilder::new()
        .call_simple(inner_addr)
        .pop()
        .return_empty()
        .finish();

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(inner_addr, &inner_revert)
        .with_evm_contract(outer_addr, &outer_bytecode);

    let tx = call_tx(signer, outer_addr, 200_000);
    let output = tester.execute_block(vec![tx]);
    assert_tx_success!(output, 0);
}
