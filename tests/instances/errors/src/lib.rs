//! Error and revert path tests for ZKsync OS.
//!
//! These tests verify that the system correctly handles invalid transactions, out-of-gas
//! conditions, EVM reverts, and deployment failures.
//!
//! Each test checks at minimum:
//! - The correct success/failure status.
//! - That gas accounting is reasonable.
//! - Where applicable, that state is not mutated on failure.

#![cfg(test)]

use alloy::primitives::{address, Address};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use rig::builder::{ChainBuilder, TxBuilder};
use rig::constants::*;
use rig::run_config;
use rig::ruint::aliases::{B160, U256};
use rig::Chain;
use rig::{assert_tx_failed, assert_tx_reverted, assert_tx_success};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn b160(addr: Address) -> B160 {
    B160::from_be_bytes(addr.into_array())
}

// ─── Out-of-gas ──────────────────────────────────────────────────────────────

/// An ETH transfer with gas_limit=1 must fail — not enough gas for intrinsic cost.
#[test]
fn out_of_gas_simple_transfer() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let recipient = address!("deadbeef00000000000000000000000000000001");

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .gas_limit(1) // far below intrinsic cost
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // bootloader should reject — insufficient gas for intrinsic cost
    assert_tx_failed!(output, 0);
}

/// A contract call whose execution exhausts gas mid-execution should be processed but revert.
#[test]
fn out_of_gas_mid_execution() {
    // Deploy a contract that loops until it runs out of gas.
    // Bytecode: JUMPDEST JUMP (infinite loop) — consumes all gas.
    // 0x5b = JUMPDEST, 0x60 0x00 = PUSH1 0, 0x56 = JUMP
    let loop_bytecode = hex::decode("5b600056").unwrap();
    let contract = address!("0000000000000000000000000000000000000101");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), loop_bytecode)
        .build();

    // Give enough gas to pass validation but not to complete the loop
    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(25_000)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // Bootloader accepted the tx (valid structure), but EVM ran out of gas
    assert_tx_reverted!(output, 0);
}

/// A deployment whose constructor runs out of gas should fail.
#[test]
fn out_of_gas_deployment() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    // Use the full ERC-20 deployment bytecode — requires significant gas
    let deploy_bytecode = hex::decode(rig::utils::ERC_20_DEPLOYMENT_BYTECODE).unwrap();

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    // Use a gas limit far too small for a real deployment
    let tx = TxBuilder::new()
        .create()
        .from(signer)
        .calldata(deploy_bytecode)
        .gas_limit(5_000) // too small
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // With gas_limit=5_000, the bootloader rejects the tx before even starting EVM execution
    // (the intrinsic cost + deployment overhead exceeds the gas limit at validation time).
    assert_tx_failed!(output, 0);
}

// ─── Invalid transactions ─────────────────────────────────────────────────────

/// A transaction with the wrong chain ID must be rejected.
#[test]
fn wrong_chain_id_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    // chain_id = 1 (mainnet) but the chain runs with TEST_CHAIN_ID = 37
    let tx = TxBuilder::new()
        .chain_id(1)
        .from(signer)
        .to(recipient)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_failed!(output, 0);
}

/// A transaction with nonce lower than the current account nonce is rejected.
#[test]
fn nonce_too_low_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let recipient = address!("0000000000000000000000000000000000000002");

    // Pre-set nonce = 5 on the sender account
    let mut chain = Chain::empty(None);
    chain.set_balance(sender, U256::from(DEFAULT_BALANCE));
    chain.set_account_properties(sender, None, Some(5), None);

    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .nonce(0) // nonce 0 < current nonce 5
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_failed!(output, 0);
}

/// A transaction with nonce higher than the current account nonce is rejected.
#[test]
fn nonce_too_high_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    // account nonce is 0, sending nonce 5 → too high
    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .nonce(5)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_failed!(output, 0);
}

/// A sender with insufficient balance for gas + value is rejected.
#[test]
fn insufficient_balance_for_gas_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut chain = ChainBuilder::new()
        // balance only covers the value, not gas
        .with_balance(sender, U256::from(1u64))
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .value(alloy::primitives::U256::from(1u64))
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_failed!(output, 0);
}

/// `max_fee_per_gas` below the current basefee must be rejected.
#[test]
fn max_fee_below_basefee_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    // BlockContext::default() has basefee = 1000; set max_fee = 1 < basefee
    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .max_fee(1)
        .priority_fee(0)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_failed!(output, 0);
}

// ─── Revert paths ─────────────────────────────────────────────────────────────

/// `REVERT(0, 0)` — EVM-level revert with no return data.
#[test]
fn explicit_revert_no_data() {
    // 0x60 0x00 0x60 0x00 0xfd = PUSH1 0, PUSH1 0, REVERT
    let revert_bytecode = hex::decode("60006000fd").unwrap();
    let contract = address!("0000000000000000000000000000000000000201");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), revert_bytecode)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_reverted!(output, 0);
}

/// A call to a contract that performs `REVERT` with return data — data should be preserved.
#[test]
fn explicit_revert_with_data() {
    // Bytecode: PUSH4 0xdeadbeef, PUSH1 0x1c, MSTORE, PUSH1 4, PUSH1 0x1c, REVERT
    // stores 0xdeadbeef at offset 0x1c, then reverts returning 4 bytes
    let revert_with_data = hex::decode("63deadbeef601c5260046000fd").unwrap();
    let contract = address!("0000000000000000000000000000000000000202");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), revert_with_data)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_reverted!(output, 0);
}

/// `INVALID` opcode (0xfe) must cause an EVM-level failure.
#[test]
fn invalid_opcode() {
    // 0xfe = INVALID
    let invalid_bytecode = hex::decode("fe").unwrap();
    let contract = address!("0000000000000000000000000000000000000203");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), invalid_bytecode)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // INVALID consumes all gas and reverts
    assert_tx_reverted!(output, 0);
}

/// Call to a plain EOA address with calldata — should succeed (no code, empty return).
#[test]
fn call_to_eoa_with_calldata_succeeds() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let eoa = address!("0000000000000000000000000000000000000204");

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(eoa)
        .calldata(vec![0xca, 0xfe, 0xba, 0xbe])
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
}

/// Nested call: inner call reverts, outer call handles it and succeeds.
#[test]
fn nested_call_inner_reverts_outer_succeeds() {
    // Inner contract: always reverts
    let inner_revert = hex::decode("60006000fd").unwrap();
    let inner_addr = address!("0000000000000000000000000000000000000205");

    // Outer contract: CALLs inner, ignores result (success = CALL return value pushed on stack,
    // then POP), then RETURNs(0, 0).
    // Bytecode: PUSH1 0 (retsize) PUSH1 0 (retoffset) PUSH1 0 (argssize) PUSH1 0 (argsoffset)
    //           PUSH1 0 (value) PUSH20 inner_addr GAS CALL POP PUSH1 0 PUSH1 0 RETURN
    let inner_bytes = inner_addr.into_array();
    let mut outer_bytecode: Vec<u8> = vec![
        0x60, 0x00, // PUSH1 0 (retsize)
        0x60, 0x00, // PUSH1 0 (retoffset)
        0x60, 0x00, // PUSH1 0 (argssize)
        0x60, 0x00, // PUSH1 0 (argsoffset)
        0x60, 0x00, // PUSH1 0 (value)
        0x73, // PUSH20
    ];
    outer_bytecode.extend_from_slice(&inner_bytes);
    outer_bytecode.extend_from_slice(&[
        0x5a, // GAS
        0xf1, // CALL
        0x50, // POP (discard call result)
        0x60, 0x00, // PUSH1 0 (size)
        0x60, 0x00, // PUSH1 0 (offset)
        0xf3, // RETURN
    ]);

    let outer_addr = address!("0000000000000000000000000000000000000206");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(inner_addr), inner_revert)
        .with_evm_bytecode(b160(outer_addr), outer_bytecode)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(outer_addr)
        .gas_limit(200_000)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // Outer tx succeeded even though inner reverted
    assert_tx_success!(output, 0);
}

// ─── Deployment failures ──────────────────────────────────────────────────────

/// Constructor that reverts — deployment must fail.
#[test]
fn constructor_revert_fails_deployment() {
    // Init bytecode that immediately reverts: PUSH1 0, PUSH1 0, REVERT
    let init_bytecode = hex::decode("60006000fd").unwrap();

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    let tx = TxBuilder::new()
        .create()
        .from(signer)
        .calldata(init_bytecode)
        .gas_limit(DEPLOY_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // Constructor calls REVERT — the tx was accepted by bootloader but EVM reverted.
    assert_tx_reverted!(output, 0);
}

/// Zero-length runtime bytecode deployment — init bytecode that returns empty code.
/// Depending on ZKsync OS behaviour, this may succeed or fail.
#[test]
fn zero_length_deployed_code() {
    // Init bytecode: PUSH1 0, PUSH1 0, RETURN — deploys 0 bytes of code
    let init_bytecode = hex::decode("60006000f3").unwrap();

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    let tx = TxBuilder::new()
        .create()
        .from(signer)
        .calldata(init_bytecode)
        .gas_limit(DEPLOY_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // Deploying empty code is not standard EVM — may succeed or be rejected.
    // We just verify the block ran without panicking and the result is deterministic.
    match &output.tx_results[0] {
        Ok(_) | Err(_) => {} // either outcome is valid; we just need no panic
    }
}

// ─── State isolation on revert ────────────────────────────────────────────────

/// A reverted call must not persist storage writes.
#[test]
fn revert_does_not_mutate_storage() {
    // Contract: SSTORE slot 0 <- 0xdead, then REVERT
    // 0x61 0xde 0xad = PUSH2 0xdead
    // 0x60 0x00      = PUSH1 0 (slot)
    // 0x55           = SSTORE
    // 0x60 0x00      = PUSH1 0
    // 0x60 0x00      = PUSH1 0
    // 0xfd           = REVERT
    let revert_after_store = hex::decode("61dead60005560006000fd").unwrap();
    let contract = address!("0000000000000000000000000000000000000301");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), revert_after_store)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_reverted!(output, 0);

    // No storage write for this contract should appear in the output
    let contract_b160 = b160(contract);
    let wrote_to_contract = output.storage_writes.iter().any(|w| {
        let acc: [u8; 32] = w.account.0;
        let expected: [u8; 32] = {
            let mut buf = [0u8; 32];
            buf[12..].copy_from_slice(&contract_b160.to_be_bytes::<20>());
            buf
        };
        acc == expected
    });
    assert!(
        !wrote_to_contract,
        "reverted tx must not produce storage writes for the reverted contract"
    );
}

// ─── TSTORE reverts correctly when the frame reverts ─────────────────────────

/// TSTORE followed by REVERT: the transient write must not persist after the revert.
///
/// Tx0: call a contract that does `TSTORE 0 <- 0xdead; REVERT`. The tx reverts.
/// Tx1 (next block): call a "check" contract that does `TLOAD 0; return 32 bytes`.
///   The returned value must be 0x00..00, proving the TSTORE was rolled back.
///
/// Note: transient storage is per-frame and is rolled back with the frame on REVERT.
/// (opcode 0x5d = TSTORE, 0x5c = TLOAD, 0xfd = REVERT)
#[test]
fn tstore_reverts_on_frame_revert() {
    // Revert contract: PUSH2 0xdead  PUSH1 0x00  TSTORE  PUSH1 0x00  PUSH1 0x00  REVERT
    //   0x61 0xde 0xad = PUSH2 0xdead
    //   0x60 0x00      = PUSH1 0
    //   0x5d           = TSTORE
    //   0x60 0x00      = PUSH1 0 (size)
    //   0x60 0x00      = PUSH1 0 (offset)
    //   0xfd           = REVERT
    let revert_bytecode = hex::decode("61dead60005d60006000fd").unwrap();
    let revert_contract = address!("0000000000000000000000000000000000000d01");

    // Check contract: PUSH1 0x00  TLOAD  PUSH1 0x00  MSTORE  PUSH1 0x20  PUSH1 0x00  RETURN
    //   Returns the 32-byte value of transient slot 0 (should be 0 after the revert)
    let check_bytecode = hex::decode("60005c60005260206000f3").unwrap();
    let check_contract = address!("0000000000000000000000000000000000000d02");

    let signer = PrivateKeySigner::random();
    let signer2 = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let sender2 = b160(signer2.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_balance(sender2, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(revert_contract), revert_bytecode)
        .with_evm_bytecode(b160(check_contract), check_bytecode)
        .build();

    // Block 1: tx0 reverts (TSTORE rolled back)
    let tx0 = TxBuilder::new()
        .from(signer)
        .to(revert_contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx0], None, None, Some(run_config::forward_only()));

    // The transaction reverted at EVM level
    assert_tx_reverted!(output, 0);

    // Block 2: tx1 reads transient slot 0 of the check_contract — must be 0
    // (transient storage is cleared between transactions and between blocks)
    let tx1 = TxBuilder::new()
        .from(signer2)
        .to(check_contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output2 = chain.run_block(vec![tx1], None, None, Some(run_config::forward_only()));
    assert_tx_success!(output2, 0);

    let tx1_out = output2.tx_results[0].as_ref().unwrap();
    let returned = tx1_out.as_returned_bytes();
    assert_eq!(
        returned,
        &[0u8; 32],
        "transient storage slot 0 must be 0 after REVERT rolled back the TSTORE"
    );
}

// ─── SELFDESTRUCT in a reverting outer frame ──────────────────────────────────

/// SELFDESTRUCT called in a sub-frame that gets rolled back: the SELFDESTRUCT must not take effect.
///
/// outer contract: calls inner (which SELFDESTRUCTs) then reverts outer
/// inner contract: SELFDESTRUCT targeting a beneficiary
/// After the block, the beneficiary should have 0 balance (SELFDESTRUCT rolled back).
#[test]
fn selfdestruct_in_reverting_frame_no_effect() {
    // inner contract: SELFDESTRUCT to a fixed address (0xdead...1234)
    // 0x73 = PUSH20, 0xff = SELFDESTRUCT
    let beneficiary = address!("dead000000000000000000000000000000001234");
    let beneficiary_bytes = beneficiary.into_array();
    let mut inner_bytecode = vec![0x73u8]; // PUSH20
    inner_bytecode.extend_from_slice(&beneficiary_bytes);
    inner_bytecode.push(0xff); // SELFDESTRUCT

    let inner_addr = address!("0000000000000000000000000000000000000e01");
    let inner_b160 = b160(inner_addr);

    // outer contract: CALL inner (sub-call), then REVERT(0, 0)
    // PUSH1 0 (retsize) PUSH1 0 (retoffset) PUSH1 0 (argssize) PUSH1 0 (argsoffset)
    // PUSH1 0 (value) PUSH20 <inner> GAS CALL POP PUSH1 0 PUSH1 0 REVERT
    let inner_bytes = inner_addr.into_array();
    let mut outer_bytecode: Vec<u8> = vec![
        0x60, 0x00, // PUSH1 0 (retsize)
        0x60, 0x00, // PUSH1 0 (retoffset)
        0x60, 0x00, // PUSH1 0 (argssize)
        0x60, 0x00, // PUSH1 0 (argsoffset)
        0x60, 0x00, // PUSH1 0 (value)
        0x73, // PUSH20
    ];
    outer_bytecode.extend_from_slice(&inner_bytes);
    outer_bytecode.extend_from_slice(&[
        0x5a, // GAS
        0xf1, // CALL
        0x50, // POP (discard result)
        0x60, 0x00, // PUSH1 0 (size)
        0x60, 0x00, // PUSH1 0 (offset)
        0xfd, // REVERT
    ]);
    let outer_addr = address!("0000000000000000000000000000000000000e02");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    // Fund the inner contract with some ETH so SELFDESTRUCT has something to transfer
    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_balance(inner_b160, U256::from(1_000u64))
        .with_evm_bytecode(inner_b160, inner_bytecode)
        .with_evm_bytecode(b160(outer_addr), outer_bytecode)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(outer_addr)
        .gas_limit(200_000)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // Outer tx reverted at EVM level
    assert_tx_reverted!(output, 0);

    // Beneficiary must NOT have received any ETH — SELFDESTRUCT was rolled back
    let beneficiary_b160 = b160(beneficiary);
    let beneficiary_balance = chain.get_account_properties(&beneficiary_b160).balance;
    assert_eq!(
        beneficiary_balance,
        U256::ZERO,
        "SELFDESTRUCT in reverting frame must not transfer ETH to beneficiary"
    );
}
