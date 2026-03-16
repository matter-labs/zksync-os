//! Rollback semantics for storage, transient storage, and selfdestruct side effects.

use crate::test_support::{call_tx, new_tester};
use rig::alloy::primitives::address;
use rig::alloy::signers::local::PrivateKeySigner;
use rig::constants::{CALL_GAS_LIMIT, DEFAULT_BALANCE};
use rig::ruint::aliases::U256;
use rig::{assert_tx_reverted, assert_tx_success};

#[test]
fn revert_does_not_mutate_storage() {
    let revert_after_store = hex::decode("61dead60005560006000fd").unwrap();
    let contract = address!("0000000000000000000000000000000000000301");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &revert_after_store);

    let tx = call_tx(signer, contract, CALL_GAS_LIMIT);
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

    let tx = call_tx(signer, outer_addr, CALL_GAS_LIMIT);
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

    let tx = call_tx(signer, outer_addr, 200_000);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);

    let beneficiary_balance = tester.get_account_properties(&beneficiary).balance;
    assert_eq!(
        beneficiary_balance,
        U256::ZERO,
        "SELFDESTRUCT in reverting frame must not transfer ETH to beneficiary"
    );
}
