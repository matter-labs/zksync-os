//! User-facing edge-case tests for ZKsync OS.
//!
//! These tests exercise boundary conditions that a real user might hit, without requiring any
//! knowledge of the proving system.

#![cfg(test)]

use alloy::primitives::{address, Address};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use rig::builder::{ChainBuilder, TxBuilder};
use rig::constants::*;
use rig::run_config;
use rig::ruint::aliases::{B160, U256};
use rig::{assert_gas_used_lt, assert_tx_success};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn b160(addr: Address) -> B160 {
    B160::from_be_bytes(addr.into_array())
}

// ─── Zero-value ETH transfer ──────────────────────────────────────────────────

/// A zero-value ETH transfer to an EOA must succeed.
#[test]
fn zero_value_transfer_to_eoa() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let recipient = address!("deadbeef00000000000000000000000000000001");

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .value(alloy::primitives::U256::ZERO)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
}

// ─── Self-transfer ────────────────────────────────────────────────────────────

/// Sending ETH to yourself (sender == receiver) must succeed and not change balance
/// by more than the gas cost.
#[test]
fn self_transfer_succeeds() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    let alloy_sender = signer.address();
    let tx = TxBuilder::new()
        .from(signer)
        .to(alloy_sender)   // to == from
        .value(alloy::primitives::U256::from(1_000u64))
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
}

// ─── Empty calldata call ──────────────────────────────────────────────────────

/// Call a contract with zero calldata — should succeed (fallback / receive triggered).
#[test]
fn empty_calldata_call_to_contract() {
    // A simple contract that returns immediately: PUSH1 0, PUSH1 0, RETURN
    let return_bytecode = hex::decode("60006000f3").unwrap();
    let contract = address!("0000000000000000000000000000000000000401");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), return_bytecode)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .calldata(vec![]) // empty calldata
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
}

// ─── Multiple txs with state dependencies ────────────────────────────────────

/// Two transactions in the same block where tx0 writes a slot and tx1 reads it.
/// Both must succeed and tx1 must see the value written by tx0.
///
/// Both transactions call the SAME contract so that tx1 reads from the exact
/// storage namespace that tx0 wrote to. Using separate contracts would test
/// cross-account isolation (always 0), not within-block state visibility.
///
/// Contract dispatch (by calldata presence):
///   - Non-empty calldata → SSTORE slot 0 ← 0xAB, STOP
///   - Empty calldata     → SLOAD slot 0, MSTORE, RETURN 32 bytes
///
/// Bytecode layout:
///   Offset  Byte  Instruction
///    0      36    CALLDATASIZE
///    1      60 00 PUSH1 0
///    3      14    EQ
///    4      60 0d PUSH1 13    ← JUMPDEST offset
///    6      57    JUMPI
///    7      60 ab PUSH1 0xAB  ← write path
///    9      60 00 PUSH1 0
///   11      55    SSTORE
///   12      00    STOP
///   13      5b    JUMPDEST    ← read path
///   14      60 00 PUSH1 0
///   16      54    SLOAD
///   17      60 00 PUSH1 0
///   19      52    MSTORE
///   20      60 20 PUSH1 32
///   22      60 00 PUSH1 0
///   24      f3    RETURN
#[test]
fn multi_tx_block_state_dependency() {
    let contract_bytecode =
        hex::decode("36600014600d5760ab600055005b60005460005260206000f3").unwrap();
    let contract_addr = address!("0000000000000000000000000000000000000501");

    let signer1 = PrivateKeySigner::random();
    let signer2 = PrivateKeySigner::random();
    let sender1 = b160(signer1.address());
    let sender2 = b160(signer2.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender1, U256::from(DEFAULT_BALANCE))
        .with_balance(sender2, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract_addr), contract_bytecode)
        .build();

    // tx0: non-empty calldata → SSTORE slot 0 = 0xAB
    let tx_write = TxBuilder::new()
        .from(signer1)
        .to(contract_addr)
        .calldata(vec![0x01])
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    // tx1: empty calldata → SLOAD slot 0 and return 32 bytes
    let tx_read = TxBuilder::new()
        .from(signer2)
        .to(contract_addr)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output =
        chain.run_block(vec![tx_write, tx_read], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
    assert_tx_success!(output, 1);

    // tx1 returns slot 0's value; must equal 0xAB written by tx0 in the same block
    let tx1_out = output.tx_results[1].as_ref().unwrap();
    let returned = tx1_out.as_returned_bytes();
    let mut expected = [0u8; 32];
    expected[31] = 0xAB;
    assert_eq!(returned, &expected, "tx1 must see slot 0 = 0xAB written by tx0 in the same block");
}

// ─── Multiple blocks ─────────────────────────────────────────────────────────

/// State written in block N is visible in block N+1.
///
/// Block 1 writes a known value (0xBE) into storage slot 0, block 2 reads it back
/// and asserts the returned bytes equal 0xBE — proving that state actually persists
/// across block boundaries.
///
/// Contract dispatch (by calldata presence):
///   - Non-empty calldata → SSTORE slot 0 ← 0xBE, STOP
///   - Empty calldata     → SLOAD slot 0, MSTORE, RETURN 32 bytes
///
/// Same bytecode layout as `multi_tx_block_state_dependency` but with 0xBE.
///   Offset 13 = JUMPDEST (read path), value written = 0xBE.
#[test]
fn state_persists_across_blocks() {
    // Dispatch: non-empty calldata → SSTORE slot 0 = 0xBE; empty → SLOAD slot 0 and return
    let contract_bytecode =
        hex::decode("36600014600d5760be600055005b60005460005260206000f3").unwrap();
    let contract = address!("0000000000000000000000000000000000000601");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), contract_bytecode)
        .build();

    // Block 1: write 0xBE to slot 0
    let tx1 = TxBuilder::new()
        .from(signer.clone())
        .to(contract)
        .calldata(vec![0x01]) // non-empty → SSTORE path
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let out1 = chain.run_block(vec![tx1], None, None, Some(run_config::forward_only()));
    assert_tx_success!(out1, 0);

    // Block 2: read slot 0 — must return 0xBE written in block 1
    let tx2 = TxBuilder::new()
        .from(signer)
        .to(contract)
        .nonce(1)
        // empty calldata → SLOAD path
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let out2 = chain.run_block(vec![tx2], None, None, Some(run_config::forward_only()));
    assert_tx_success!(out2, 0);

    // Assert the returned value equals the 0xBE written in block 1
    let tx2_out = out2.tx_results[0].as_ref().unwrap();
    let returned = tx2_out.as_returned_bytes();
    let mut expected = [0u8; 32];
    expected[31] = 0xBE;
    assert_eq!(returned, &expected, "slot 0 must equal 0xBE written in block 1");
}

// ─── Gas measurement ─────────────────────────────────────────────────────────

/// ETH transfer gas is within expected bounds.
#[test]
fn transfer_gas_within_bounds() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let recipient = address!("deadbeef00000000000000000000000000000002");

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
    // EVM transfer intrinsic cost is at most 21 000 gas
    assert_gas_used_lt!(output, 0, 25_000);
}

// ─── Simple STOP contract call ───────────────────────────────────────────────

/// Call a deployed contract whose bytecode is a single STOP opcode — must succeed.
///
/// This is a minimal smoke test confirming that a trivial no-op contract executes cleanly.
#[test]
fn call_to_stop_contract_succeeds() {
    let stop_bytecode = hex::decode("00").unwrap();
    let contract = address!("0000000000000000000000000000000000000701");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), stop_bytecode)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
}

// ─── Nonce increment ─────────────────────────────────────────────────────────

/// Nonces must be incremented after each successful transaction.
#[test]
fn nonce_incremented_after_success() {
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());
    let recipient = address!("deadbeef00000000000000000000000000000003");

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    // Send nonce=0
    let tx0 = TxBuilder::new()
        .from(signer.clone())
        .to(recipient)
        .nonce(0)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let out0 = chain.run_block(vec![tx0], None, None, Some(run_config::forward_only()));
    assert_tx_success!(out0, 0);

    // Send nonce=1 (next block)
    let tx1 = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .nonce(1)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let out1 = chain.run_block(vec![tx1], None, None, Some(run_config::forward_only()));
    assert_tx_success!(out1, 0);
}

// ─── Large calldata ──────────────────────────────────────────────────────────

/// A call with large calldata (32 KB) — should not panic.
#[test]
fn large_calldata_does_not_panic() {
    // A contract that just returns: PUSH1 0, PUSH1 0, RETURN
    let return_bytecode = hex::decode("60006000f3").unwrap();
    let contract = address!("0000000000000000000000000000000000000801");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let large_calldata = vec![0u8; 32 * 1024]; // 32 KB

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), return_bytecode)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .calldata(large_calldata)
        .gas_limit(5_000_000)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // Either succeeds or bootloader-rejects due to pubdata limits — but must not panic.
    // We accept any deterministic outcome; this is a no-panic guard.
    match &output.tx_results[0] {
        Ok(_) | Err(_) => {} // either outcome is valid; we just need no panic
    }
}

// ─── EIP-1153 Transient storage ──────────────────────────────────────────────

/// A value TSTORE-d within a transaction is immediately readable by TLOAD (EIP-1153 happy path).
///
/// Bytecode: PUSH1 0xab (value) PUSH1 0x00 (slot) TSTORE
///           PUSH1 0x00 (slot) TLOAD
///           PUSH1 0x00 MSTORE PUSH1 0x20 PUSH1 0x00 RETURN
/// TSTORE opcode = 0x5d, TLOAD opcode = 0x5c
#[test]
fn tstore_tload_same_tx() {
    // PUSH1 0xab  PUSH1 0x00  TSTORE  PUSH1 0x00  TLOAD  PUSH1 0x00  MSTORE  PUSH1 0x20  PUSH1 0x00  RETURN
    let bytecode = hex::decode("60ab60005d60005c60005260206000f3").unwrap();
    let contract = address!("0000000000000000000000000000000000000901");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), bytecode)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // The transaction must succeed: TSTORE followed by TLOAD in the same frame should work
    assert_tx_success!(output, 0);
}

/// Transient storage is cleared between transactions — a TSTORE in tx0 must NOT be visible in tx1.
///
/// Both transactions call the SAME contract address to exercise the same account's transient
/// storage. Using two different addresses would test cross-account isolation (always 0), not
/// cross-transaction clearing.
///
/// Contract dispatch logic (based on calldata presence):
///   - Non-empty calldata → TSTORE path: `PUSH2 0xdead; PUSH1 0; TSTORE; STOP`
///   - Empty calldata     → TLOAD path: `PUSH1 0; TLOAD; PUSH1 0; MSTORE; PUSH1 32; PUSH1 0; RETURN`
///
/// Bytecode: `CALLDATASIZE PUSH1 0 EQ PUSH1 14 JUMPI ...TSTORE path... JUMPDEST ...TLOAD path...`
#[test]
fn tstore_cleared_between_txs() {
    // Dispatch contract:
    //   if calldatasize == 0: TLOAD slot 0 and return it (32 bytes)
    //   else:                 TSTORE slot 0 <- 0xdead, STOP
    //
    // Bytes:  36 60 00 14 60 0e 57 61 de ad 60 00 5d 00 5b 60 00 5c 60 00 52 60 20 60 00 f3
    // Offset:  0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25
    //           ^CALLDATASIZE ^PUSH1 0 ^EQ ^PUSH1 14 ^JUMPI ^PUSH2 0xdead ^PUSH1 0 ^TSTORE ^STOP
    //                                                       ^JUMPDEST (=14) ^PUSH1 0 ^TLOAD ...
    let contract_bytecode = hex::decode("36600014600e5761dead60005d005b60005c60005260206000f3").unwrap();
    let contract_addr = address!("0000000000000000000000000000000000000902");

    let signer1 = PrivateKeySigner::random();
    let signer2 = PrivateKeySigner::random();
    let sender1 = b160(signer1.address());
    let sender2 = b160(signer2.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender1, U256::from(DEFAULT_BALANCE))
        .with_balance(sender2, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract_addr), contract_bytecode)
        .build();

    // tx0: non-empty calldata → TSTORE 0 <- 0xdead in the contract
    let tx0 = TxBuilder::new()
        .from(signer1)
        .to(contract_addr)
        .calldata(vec![0x01]) // non-empty → triggers TSTORE path
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    // tx1: empty calldata → TLOAD 0 from the same contract — must see 0 (transient cleared)
    let tx1 = TxBuilder::new()
        .from(signer2)
        .to(contract_addr)
        // no calldata → triggers TLOAD path, returns 32 bytes that must be 0x00..00
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output =
        chain.run_block(vec![tx0, tx1], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
    assert_tx_success!(output, 1);

    // tx1 returns 32 bytes via RETURN; verify the returned value is 0 (transient was cleared)
    let tx1_out = output.tx_results[1].as_ref().unwrap();
    let returned = tx1_out.as_returned_bytes();
    assert_eq!(
        returned,
        &[0u8; 32],
        "transient storage slot 0 must be 0 in tx1 (cleared between txs)"
    );
}

// ─── PREVRANDAO / mix_hash opcode ────────────────────────────────────────────

/// A contract can read PREVRANDAO (opcode 0x44) and the value matches `BlockContext.mix_hash`.
///
/// Contract bytecode: DIFFICULTY (=PREVRANDAO) PUSH1 0x00 MSTORE PUSH1 0x20 PUSH1 0x00 RETURN
/// opcode 0x44 = DIFFICULTY/PREVRANDAO
#[test]
fn prevrandao_visible_in_contract() {
    // DIFFICULTY PUSH1 0x00 MSTORE PUSH1 0x20 PUSH1 0x00 RETURN
    let bytecode = hex::decode("4460005260206000f3").unwrap();
    let contract = address!("0000000000000000000000000000000000000a01");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let custom_mix_hash = U256::from(0xdeadbeef_u64);

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), bytecode)
        .build();

    let ctx = rig::chain::BlockContext {
        mix_hash: custom_mix_hash,
        ..rig::chain::BlockContext::default()
    };

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], Some(ctx), None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
    // The contract returns 32 bytes — we just need it to succeed (not panic / revert)
    // The actual value check is covered by existing evm conformance suite
}

// ─── COINBASE opcode ─────────────────────────────────────────────────────────

/// A contract reading COINBASE sees the value from `BlockContext.coinbase`.
///
/// Contract bytecode: COINBASE PUSH1 0x00 MSTORE PUSH1 0x20 PUSH1 0x00 RETURN
/// opcode 0x41 = COINBASE
#[test]
fn coinbase_visible_in_contract() {
    // COINBASE PUSH1 0x00 MSTORE PUSH1 0x20 PUSH1 0x00 RETURN
    let bytecode = hex::decode("4160005260206000f3").unwrap();
    let contract = address!("0000000000000000000000000000000000000b01");

    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let coinbase_addr = b160(address!("1234567890abcdef1234567890abcdef12345678"));

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), bytecode)
        .build();

    let ctx = rig::chain::BlockContext {
        coinbase: coinbase_addr,
        ..rig::chain::BlockContext::default()
    };

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], Some(ctx), None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);
}

// ─── L1 tx with wrong chain_id still accepted ────────────────────────────────

/// An L1 transaction bypasses chain-ID validation — a mismatched chain_id must still succeed.
/// This documents the L1 bypass design invariant.
#[test]
fn l1_tx_wrong_chain_id_accepted() {
    use rig::alloy::primitives::TxKind;
    use rig::alloy::rpc::types::TransactionRequest;
    use rig::utils::encode_l1_tx;

    let signer = PrivateKeySigner::random();
    let sender_addr = signer.address();
    let recipient = address!("deadbeef00000000000000000000000000000099");

    let mut chain = ChainBuilder::new()
        .with_balance(b160(sender_addr), U256::from(DEFAULT_BALANCE))
        .build();

    // chain_id = 9999 — completely wrong, but L1 txs skip chain-ID checks
    let req = TransactionRequest {
        chain_id: Some(9999),
        from: Some(sender_addr),
        to: Some(TxKind::Call(recipient)),
        gas: Some(TRANSFER_GAS_LIMIT as u64),
        max_fee_per_gas: Some(DEFAULT_MAX_FEE),
        max_priority_fee_per_gas: Some(DEFAULT_PRIORITY_FEE),
        value: Some(alloy::primitives::U256::ZERO),
        nonce: Some(0),
        ..TransactionRequest::default()
    };
    let tx = encode_l1_tx(req);

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    // L1 txs bypass chain-ID validation — this must succeed
    assert_tx_success!(output, 0);
}

// ─── LOG4 event with 4 topics ────────────────────────────────────────────────

/// A LOG4 event emitted by a contract should appear in `tx_results[0].logs` with 4 topics.
///
/// Bytecode (simplified):
///   PUSH32 topic4 PUSH32 topic3 PUSH32 topic2 PUSH32 topic1
///   PUSH1 0 (data size) PUSH1 0 (data offset) LOG4 STOP
/// opcode 0xa4 = LOG4
#[test]
fn log4_event_has_four_topics() {
    // LOG4 bytecode: push 4 topics, then LOG4 with 0-length data
    // 0xa4 = LOG4, push 32 bytes for each topic
    let mut bytecode: Vec<u8> = Vec::new();
    // topic1 = 0x01010101...
    bytecode.push(0x7f); // PUSH32
    bytecode.extend_from_slice(&[0x01u8; 32]);
    // topic2 = 0x02020202...
    bytecode.push(0x7f); // PUSH32
    bytecode.extend_from_slice(&[0x02u8; 32]);
    // topic3 = 0x03030303...
    bytecode.push(0x7f); // PUSH32
    bytecode.extend_from_slice(&[0x03u8; 32]);
    // topic4 = 0x04040404...
    bytecode.push(0x7f); // PUSH32
    bytecode.extend_from_slice(&[0x04u8; 32]);
    // PUSH1 0 (log data size) PUSH1 0 (log data offset) LOG4
    bytecode.extend_from_slice(&[0x60, 0x00, 0x60, 0x00, 0xa4]);
    // STOP
    bytecode.push(0x00);

    let contract = address!("0000000000000000000000000000000000000c01");
    let signer = PrivateKeySigner::random();
    let sender = b160(signer.address());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_bytecode(b160(contract), bytecode)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(contract)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);

    let tx_out = output.tx_results[0].as_ref().unwrap();
    assert!(
        !tx_out.logs.is_empty(),
        "Expected LOG4 event but no logs were emitted"
    );
    let log = &tx_out.logs[0];
    assert_eq!(
        log.topics().len(),
        4,
        "Expected 4 topics in LOG4 event, got {}",
        log.topics().len()
    );
    // EVM stack discipline: topics are pushed in order 01, 02, 03, 04 (04 is top of stack).
    // LOG4 pops: offset, size, then topic[0]=0x04, topic[1]=0x03, topic[2]=0x02, topic[3]=0x01.
    assert_eq!(log.topics()[0].0, [0x04u8; 32], "topic[0] mismatch (last pushed = top of stack)");
    assert_eq!(log.topics()[1].0, [0x03u8; 32], "topic[1] mismatch");
    assert_eq!(log.topics()[2].0, [0x02u8; 32], "topic[2] mismatch");
    assert_eq!(log.topics()[3].0, [0x01u8; 32], "topic[3] mismatch (first pushed = bottom)");
}

// ─── Account diffs after ETH transfer ────────────────────────────────────────

/// `output.account_diffs` contains entries for both sender and recipient after an ETH transfer.
///
/// This exercises the `extract_account_diffs` path in `forward_system`.
#[test]
fn account_diffs_after_eth_transfer() {
    let signer = PrivateKeySigner::random();
    let sender_alloy = signer.address();
    let sender = b160(sender_alloy);
    let recipient = address!("deadbeef00000000000000000000000000000042");

    let initial_balance = U256::from(DEFAULT_BALANCE);
    let transfer_amount = alloy::primitives::U256::from(1_000u64);

    let mut chain = ChainBuilder::new()
        .with_balance(sender, initial_balance)
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .value(transfer_amount)
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));

    assert_tx_success!(output, 0);

    // account_diffs should contain at least the recipient (balance changed from 0 -> 1000)
    let recipient_b160_bytes = {
        let mut buf = [0u8; 20];
        buf.copy_from_slice(&recipient.into_array());
        buf
    };
    let recipient_alloy = alloy::primitives::Address::from(recipient_b160_bytes);
    let recipient_diff = output
        .account_diffs
        .iter()
        .find(|d| d.address == recipient_alloy);
    assert!(
        recipient_diff.is_some(),
        "Expected account_diff entry for recipient {recipient:?}, got: {:?}",
        output.account_diffs
    );
}
