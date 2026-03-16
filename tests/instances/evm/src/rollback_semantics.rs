//! Rollback semantics for storage, transient storage, and selfdestruct side effects.

use crate::test_support::{call_tx, new_tester};
use rig::alloy::primitives::address;
use rig::alloy::signers::local::PrivateKeySigner;
use rig::constants::{CALL_GAS_LIMIT, DEFAULT_BALANCE};
use rig::evm_bytecode::{self, BytecodeBuilder};
use rig::ruint::aliases::U256;
use rig::{assert_tx_reverted, assert_tx_success};

#[test]
fn revert_does_not_mutate_storage() {
    let revert_after_store = evm_bytecode::sstore_u16_then_revert(0, 0xdead);
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
    let inner_bytecode = BytecodeBuilder::new()
        .calldatasize()
        .push0()
        .eq()
        .push_u8(0x0d)
        .jumpi()
        .push_u8(1)
        .push0()
        .tstore()
        .push0()
        .push0()
        .revert()
        .jumpdest()
        .push0()
        .tload()
        .push0()
        .mstore()
        .push_u8(0x20)
        .push0()
        .return_()
        .finish();
    let inner_addr = address!("0000000000000000000000000000000000000d11");

    let outer_addr = address!("0000000000000000000000000000000000000d12");
    let outer_bytecode = BytecodeBuilder::new()
        .push_u8(1)
        .push0()
        .mstore8()
        .call_with_gas(inner_addr, 0, 1, 0, 0)
        .pop()
        .call_with_gas(inner_addr, 0, 0, 0, 0x20)
        .pop()
        .push_u8(0x20)
        .push0()
        .return_()
        .finish();

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
        "transient storage written in a reverted inner frame must be rolled back before a second call to the same contract"
    );
}

#[test]
fn selfdestruct_in_reverting_frame_no_effect() {
    let beneficiary = address!("dead000000000000000000000000000000001234");
    let inner_bytecode = evm_bytecode::selfdestruct(beneficiary);
    let inner_addr = address!("0000000000000000000000000000000000000e01");

    let outer_addr = address!("0000000000000000000000000000000000000e02");
    let outer_bytecode = BytecodeBuilder::new()
        .call_simple(inner_addr)
        .pop()
        .push0()
        .push0()
        .revert()
        .finish();

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
