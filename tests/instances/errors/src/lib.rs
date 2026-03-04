//! Error and revert path tests for ZKsync OS.

#![cfg(test)]

use rig::alloy::primitives::{address, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::builder::TxBuilder;
use rig::constants::*;
use rig::ruint::aliases::U256;
use rig::run_config;
use rig::TestingFramework;
use rig::{assert_tx_failed, assert_tx_reverted, assert_tx_success};

fn new_tester() -> TestingFramework<false> {
    TestingFramework::new().with_run_config(run_config::forward_only())
}

#[test]
fn out_of_gas_simple_transfer() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("deadbeef00000000000000000000000000000001");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .gas_limit(1)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_failed!(output, 0);
}

#[test]
fn out_of_gas_mid_execution() {
    let loop_bytecode = hex::decode("5b600056").unwrap();
    let contract = address!("0000000000000000000000000000000000000101");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &loop_bytecode);

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(25_000)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn out_of_gas_deployment() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let deploy_bytecode = hex::decode(rig::utils::ERC_20_DEPLOYMENT_BYTECODE).unwrap();

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = TxBuilder::new()
        .create()
        .from(signer)
        .calldata(deploy_bytecode)
        .gas_limit(5_000)
        .build();

    let output = tester.execute_block(vec![tx]);
    assert_tx_failed!(output, 0);
}

#[test]
fn wrong_chain_id_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = TxBuilder::new()
        .chain_id(1)
        .from(signer)
        .to(recipient)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_failed!(output, 0);
}

#[test]
fn nonce_too_low_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx0 = TxBuilder::new()
        .from(signer.clone())
        .to(recipient)
        .nonce(0)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();
    let out0 = tester.execute_block(vec![tx0]);
    assert_tx_success!(out0, 0);

    let tx_low = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .nonce(0)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();
    let out1 = tester.execute_block(vec![tx_low]);
    assert_tx_failed!(out1, 0);
}

#[test]
fn nonce_too_high_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));
    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .nonce(5)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_failed!(output, 0);
}

#[test]
fn insufficient_balance_for_gas_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(1u64));
    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .value(AlloyU256::from(1u64))
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_failed!(output, 0);
}

#[test]
fn max_fee_below_basefee_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));
    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .max_fee(1)
        .priority_fee(0)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_failed!(output, 0);
}

#[test]
fn explicit_revert_no_data() {
    let revert_bytecode = hex::decode("60006000fd").unwrap();
    let contract = address!("0000000000000000000000000000000000000201");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &revert_bytecode);

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn explicit_revert_with_data() {
    let revert_with_data = hex::decode("63deadbeef6000526004601cfd").unwrap();
    let contract = address!("0000000000000000000000000000000000000202");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &revert_with_data);

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);

    let tx_out = output.tx_results[0].as_ref().unwrap();
    match &tx_out.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Revert(data) => {
            assert_eq!(data, &hex::decode("deadbeef").unwrap());
        }
        _ => panic!("expected revert with payload"),
    }
}

#[test]
fn invalid_opcode() {
    let invalid_bytecode = hex::decode("fe").unwrap();
    let contract = address!("0000000000000000000000000000000000000203");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &invalid_bytecode);

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn call_to_eoa_with_calldata_succeeds() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let eoa = address!("0000000000000000000000000000000000000204");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = TxBuilder::new()
        .from(signer)
        .to(eoa)
        .calldata(vec![0xca, 0xfe, 0xba, 0xbe])
        .gas_limit(CALL_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_success!(output, 0);
}

#[test]
fn nested_call_inner_reverts_outer_succeeds() {
    let inner_revert = hex::decode("60006000fd").unwrap();
    let inner_addr = address!("0000000000000000000000000000000000000205");

    let inner_bytes = inner_addr.into_array();
    let mut outer_bytecode: Vec<u8> = vec![
        0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x73,
    ];
    outer_bytecode.extend_from_slice(&inner_bytes);
    outer_bytecode.extend_from_slice(&[0x5a, 0xf1, 0x50, 0x60, 0x00, 0x60, 0x00, 0xf3]);

    let outer_addr = address!("0000000000000000000000000000000000000206");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(inner_addr, &inner_revert)
        .with_evm_contract(outer_addr, &outer_bytecode);

    let tx = TxBuilder::new()
        .from(signer)
        .to(outer_addr)
        .gas_limit(200_000)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_success!(output, 0);
}

#[test]
fn constructor_revert_fails_deployment() {
    let init_bytecode = hex::decode("60006000fd").unwrap();

    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = TxBuilder::new()
        .create()
        .from(signer)
        .calldata(init_bytecode)
        .gas_limit(DEPLOY_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn zero_length_deployed_code() {
    let init_bytecode = hex::decode("60006000f3").unwrap();

    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = TxBuilder::new()
        .create()
        .from(signer)
        .calldata(init_bytecode)
        .gas_limit(DEPLOY_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);
    assert_eq!(output.tx_results.len(), 1);
    assert_tx_success!(output, 0);

    let tx_out = output.tx_results[0].as_ref().unwrap();
    match &tx_out.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(
            rig::zksync_os_interface::types::ExecutionOutput::Create(data, address),
        ) => {
            assert!(data.is_empty(), "runtime code must be empty");
            assert_ne!(
                *address,
                address!("0000000000000000000000000000000000000000")
            );
        }
        _ => panic!("expected successful create execution output"),
    }
}

#[test]
fn revert_does_not_mutate_storage() {
    let revert_after_store = hex::decode("61dead60005560006000fd").unwrap();
    let contract = address!("0000000000000000000000000000000000000301");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &revert_after_store);

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();
    let output = tester.execute_block(vec![tx]);

    assert_tx_reverted!(output, 0);
    let wrote_to_contract = output.storage_writes.iter().any(|w| w.account == contract);
    assert!(
        !wrote_to_contract,
        "reverted tx must not produce storage writes for the reverted contract"
    );
}

#[test]
fn tstore_reverts_on_frame_revert() {
    let inner_bytecode = hex::decode("600160005d60006000fd").unwrap();
    let inner_addr = address!("0000000000000000000000000000000000000d11");

    let inner_bytes = inner_addr.into_array();
    let mut outer_bytecode: Vec<u8> = vec![
        0x60, 0x00, // out_size
        0x60, 0x00, // out_offset
        0x60, 0x00, // in_size
        0x60, 0x00, // in_offset
        0x60, 0x00, // value
        0x73, // push20(inner)
    ];
    outer_bytecode.extend_from_slice(&inner_bytes);
    outer_bytecode.extend_from_slice(&[
        0x5a, // gas
        0xf1, // call
        0x50, // pop(success)
        0x60, 0x00, // key = 0
        0x5c, // tload
        0x60, 0x00, // mem offset
        0x52, // mstore
        0x60, 0x20, // size
        0x60, 0x00, // offset
        0xf3, // return
    ]);
    let outer_addr = address!("0000000000000000000000000000000000000d12");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(inner_addr, &inner_bytecode)
        .with_evm_contract(outer_addr, &outer_bytecode);

    let tx = TxBuilder::new()
        .from(signer)
        .to(outer_addr)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = tester.execute_block(vec![tx]);
    assert_tx_success!(output, 0);

    let tx_out = output.tx_results[0].as_ref().unwrap();
    let returned = tx_out.as_returned_bytes();
    assert_eq!(
        returned, &[0u8; 32],
        "transient storage written in a reverted inner frame must be rolled back"
    );
}

#[test]
fn selfdestruct_in_reverting_frame_no_effect() {
    let beneficiary = address!("dead000000000000000000000000000000001234");
    let beneficiary_bytes = beneficiary.into_array();
    let mut inner_bytecode = vec![0x73u8];
    inner_bytecode.extend_from_slice(&beneficiary_bytes);
    inner_bytecode.push(0xff);

    let inner_addr = address!("0000000000000000000000000000000000000e01");

    let inner_bytes = inner_addr.into_array();
    let mut outer_bytecode: Vec<u8> = vec![
        0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x73,
    ];
    outer_bytecode.extend_from_slice(&inner_bytes);
    outer_bytecode.extend_from_slice(&[0x5a, 0xf1, 0x50, 0x60, 0x00, 0x60, 0x00, 0xfd]);
    let outer_addr = address!("0000000000000000000000000000000000000e02");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_balance(inner_addr, U256::from(1_000u64))
        .with_evm_contract(inner_addr, &inner_bytecode)
        .with_evm_contract(outer_addr, &outer_bytecode);

    let tx = TxBuilder::new()
        .from(signer)
        .to(outer_addr)
        .gas_limit(200_000)
        .build();

    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);

    let beneficiary_balance = tester.get_account_properties(&beneficiary).balance;
    assert_eq!(
        beneficiary_balance,
        U256::ZERO,
        "SELFDESTRUCT in reverting frame must not transfer ETH to beneficiary"
    );
}
