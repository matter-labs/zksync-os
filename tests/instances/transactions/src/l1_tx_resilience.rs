//!
//! Regression tests for L1 transaction processing resilience.
//!
//! These tests verify that L1 transactions are processed gracefully even when
//! certain validation constraints are violated. This is important because
//! L1 transactions cannot be invalidated (doing so would halt the chain due
//! to the priority queue).
//!
//! The scenarios tested here would have caused validation errors prior to the
//! resilience changes, but now use saturating arithmetic to allow processing
//! to continue.
//!

use rig::alloy::primitives::address;
use rig::evm_bytecode::BytecodeBuilder;
use rig::ruint::aliases::U256;
use rig::tx_succeeded;
use rig::utils::L1TxBuilder;
use rig::zksync_os_interface::types::{ExecutionOutput, ExecutionResult};
use rig::{alloy, TestingFramework};

use super::common_target_address;

/// Test that an L1 transaction with gas limit below intrinsic gas (21k) is
/// processed gracefully instead of causing a validation error.
///
/// Prior to the resilience changes, this would fail with a validation error
/// because gas_limit < intrinsic_gas. Now, saturating arithmetic is used
/// and the transaction proceeds.
#[test]
fn test_l1_tx_gas_limit_below_intrinsic() {
    let from = address!("1234000000000000000000000000000000000000");
    let to = common_target_address();

    // Create an L1 transaction with gas limit below intrinsic gas (21000)
    // The intrinsic gas for L1 txs is L1_TX_INTRINSIC_L2_GAS = 21_000
    let tx = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(15_000)
        .gas_limit(20_000)
        .value(alloy::primitives::U256::from(100))
        .build()
        .into();

    // The block should complete without panicking (no internal error)
    let mut tester = TestingFramework::new().with_balance(from, U256::from(u64::MAX));
    let result = tester.execute_block_no_panic(vec![tx]);
    assert!(
        result.is_ok(),
        "Block should complete without internal error, got: {:?}",
        result.err()
    );

    // The transaction should be processed (L1 txs cannot be invalidated)
    let output = result.unwrap();
    let tx_result = output.tx_results.first().expect("Should have tx result");
    assert!(
        tx_result.is_ok(),
        "L1 tx should be processed (not rejected with validation error), got: {:?}",
        tx_result
    );

    // The execution doesn't fail, as it doesn't consume non-intrinsic gas
    let tx_output = tx_result.as_ref().unwrap();
    assert!(tx_output.is_success(), "Transaction should succeed");
}

/// Test that an L1 transaction with an absurdly high gas price is processed
/// gracefully.
///
/// `native_per_gas` for L1 txs is now a fixed constant
/// (`L1_TX_NATIVE_PER_GAS`), independent of `gas_price`, so the historic
/// overflow path (`gas_price.div_ceil(L1_TX_NATIVE_PRICE)` exceeding u64)
/// no longer exists. The test still serves as a regression check that
/// large `gas_price` values don't break L1 tx processing through other
/// code paths (e.g. `tx_internal_cost = gas_price · gas_limit`).
#[test]
fn test_l1_tx_gas_price_overflow_native_per_gas() {
    let from = address!("1234000000000000000000000000000000000000");
    let to = common_target_address();

    // Historic threshold for overflowing `gas_price / L1_TX_NATIVE_PRICE`
    // when L1_TX_NATIVE_PRICE was 10. Kept as a witness value for the
    // regression scenario it covers.
    let overflow_gas_price = u128::from(u64::MAX) * 11;

    let tx = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(overflow_gas_price)
        .gas_limit(100_000)
        .value(alloy::primitives::U256::from(100))
        .build()
        .into();

    let mut tester =
        TestingFramework::new().with_balance(from, U256::from(1_000_000_000_000_000_u64));

    // The block should complete without panicking (no internal error)
    let result = tester.execute_block_no_panic(vec![tx]);
    assert!(
        result.is_ok(),
        "Block should complete without internal error, got: {:?}",
        result.err()
    );

    // The transaction should be processed (L1 txs cannot be invalidated)
    let output = result.unwrap();
    let tx_result = output.tx_results.first().expect("Should have tx result");
    assert!(
        tx_result.is_ok(),
        "L1 tx should be processed (not rejected with validation error), got: {:?}",
        tx_result
    );
}

#[test]
fn test_l1_tx_intrinsic_gas_overflow() {
    let from_address = address!("1234000000000000000000000000000000000000");
    let to_address = common_target_address();

    // Create an L1 transaction that will cause gas overflow
    // L1 transactions bypass the intrinsic gas check that would normally prevent this
    let overflow_l1_tx = L1TxBuilder::new()
        .from(from_address)
        .to(to_address)
        .gas_price(1000)
        .gas_limit(200000) // Gas limit that should not be sufficient for the input data
        .value(alloy::primitives::U256::from(100))
        .input(vec![0u8; 50_000].into()) // Very large input data to increase intrinsic cost
        .build()
        .into();

    // Test L1 transaction - this triggers the overflow scenario
    let mut tester =
        TestingFramework::new().with_balance(from_address, U256::from(1_000_000_000_000_000_u64));
    let result_l1 = tester.execute_block(vec![overflow_l1_tx]);

    assert!(result_l1.tx_results[0].is_ok());

    let res = result_l1.tx_results[0].as_ref().unwrap();
    assert!(
        res.is_success(),
        "This L1 transaction with gas overflow should not be reverted"
    );
}

/// L1->L2 transactions with gas_price == 0 must be free (no fee deducted from sender).
/// The effective gas price comes from tx.max_fee_per_gas, not the block base fee.
#[test]
fn test_l1_tx_zero_gas_price_is_free() {
    let sender = address!("1234000000000000000000000000000000000000");
    let recipient = common_target_address();
    let initial_balance = alloy::primitives::U256::from(1_000_000u64);

    let mut tester = TestingFramework::new().with_balance(sender, initial_balance);

    let tx = L1TxBuilder::new()
        .from(sender)
        .to(recipient)
        .gas_price(0)
        .gas_limit(200_000)
        .nonce(0)
        .build();

    let output = tester.execute_block(vec![tx]);
    assert!(
        tx_succeeded(&output, 0),
        "L1 tx with gas_price=0 must succeed"
    );

    // With gas_price == 0, no fees should be deducted from sender.
    assert_eq!(
        tester.get_balance(&sender),
        initial_balance,
        "sender balance must not change when gas_price is 0"
    );
}

/// L1->L2 tx fee behavior is independent of block base_fee.
/// The gas_used must be the same regardless of whether base_fee is 0 or non-zero,
/// because L1->L2 txs use their own gas_price from the transaction.
#[test]
fn test_l1_tx_fee_independent_of_block_base_fee() {
    let sender = address!("1234000000000000000000000000000000000000");
    let recipient = common_target_address();

    let run_l1_tx_with_base_fee = |base_fee: u64| -> u64 {
        let mut tester = TestingFramework::new()
            .with_balance(sender, U256::from(1_000_000_000_000u64))
            .with_block_context(super::BlockContext {
                eip1559_basefee: U256::from(base_fee),
                ..super::BlockContext::default()
            });

        let tx = L1TxBuilder::new()
            .from(sender)
            .to(recipient)
            .gas_price(1000)
            .gas_limit(200_000)
            .nonce(0)
            .build();

        let output = tester.execute_block(vec![tx]);
        assert!(tx_succeeded(&output, 0));
        output.tx_results[0].as_ref().unwrap().gas_used
    };

    let gas_used_zero = run_l1_tx_with_base_fee(0);
    let gas_used_high = run_l1_tx_with_base_fee(5000);

    // L1->L2 txs use their own gas_price, so gas_used should be identical
    // regardless of block base_fee
    assert_eq!(
        gas_used_zero, gas_used_high,
        "L1->L2 tx gas_used must be independent of block base_fee"
    );
}

/// Verify that an L1 transaction can consume close to its full
/// gas-implied pubdata budget under production parameters, regardless of
/// L1 `gas_price`.
///
/// Production values used here:
/// - `gas_limit = 72_000_000` (PRIORITY_TX_MAX_GAS_LIMIT)
/// - `gas_per_pubdata = 800`
/// → theoretical pubdata budget = `72_000_000 / 800 = 90_000` bytes.
///
/// Pre-fix (with `L1_TX_NATIVE_PRICE = 10` and `native_per_gas` derived
/// from `gas_price`), `native_per_pubdata = gas_per_pubdata · native_per_gas`
/// could grow large. At refund time the bootloader computes
/// `pubdata_used · native_per_pubdata` to charge native for pubdata; for a
/// tx that produced this many pubdata bytes the multiplication overflows
/// `u64` and returns an `out_of_native_resources` error from
/// `get_resources_to_charge_for_pubdata`, marking the L1 tx as reverted
/// despite the gas math saying the budget covers it.
///
/// Numerically, with the pre-fix code and `gas_price = 10^15`:
///   native_per_gas    = 10^15 / 10                 = 10^14
///   native_per_pubdata = 800 · 10^14               = 8·10^16
///   pubdata_used · native_per_pubdata ≈ 86_000 · 8·10^16 ≈ 6.88·10^21
/// which overflows u64 (max ≈ 1.8·10^19).
///
/// Post-fix `L1_TX_NATIVE_PER_GAS = 1e8` is a fixed constant, so the
/// conversion factor cancels: pubdata cost in native scales with pubdata
/// cost in gas, and any pubdata volume the gas budget covers is also
/// affordable in native.
///
/// The test sends a single L1 tx that calls the L1 messenger with a
/// ~86 KB payload — each message data byte flows through to pubdata via
/// the L2→L1 log storage, so this consumes ~86_000 of the 90_000-byte
/// theoretical budget. The gas headroom (~1.5M gas) covers intrinsic
/// (21k + zero-byte calldata at 4 gas/byte) and the L1 messenger
/// contract's `keccak256` + `LOG3` + counter SSTORE.
#[test]
fn test_l1_tx_can_use_full_pubdata_budget() {
    let from = address!("1234000000000000000000000000000000000000");
    // L1 messenger system contract address (`sendToL1(bytes)` selector
    // 0x62f84b24 emits the data as an L2→L1 message).
    let l1_messenger = rig::alloy::primitives::Address::from_slice(
        address!("0000000000000000000000000000000000008008").as_slice(),
    );

    // PRIORITY_TX_MAX_GAS_LIMIT — production cap for L1 priority txs.
    let gas_limit: u64 = 72_000_000;
    let gas_per_pubdata: u64 = 800;
    // Theoretical pubdata budget (in bytes).
    let theoretical_budget = gas_limit / gas_per_pubdata; // = 90_000

    // Payload sized to consume most of the budget while leaving gas headroom
    // for intrinsic + the L1 messenger contract's EVM ops:
    //   intrinsic ≈ 21k + 4 · (payload + ABI overhead)
    //   L1 messenger ≈ 770k (LOG3 8 gas/byte dominates) for an 86k payload
    //   pubdata ≈ 86_000 · 800 = 68_800_000
    //   total ≈ 70.0M ≤ 72M ✓
    let payload_len: usize = 86_000;
    // Use zero bytes so calldata intrinsic is 4 gas/byte (vs 16 for non-zero).
    let payload = vec![0u8; payload_len];

    // ABI calldata for `sendToL1(bytes)`: selector || offset(0x20) || length || data || pad
    let mut calldata = Vec::with_capacity(4 + 64 + payload_len.next_multiple_of(32));
    calldata.extend_from_slice(&hex::decode("62f84b24").unwrap()); // selector
    calldata.extend_from_slice(&[0u8; 32 - 1]); // offset upper 31 bytes
    calldata.push(0x20); // offset = 32
    calldata.extend_from_slice(&[0u8; 32 - 8]); // length upper 24 bytes
    calldata.extend_from_slice(&(payload_len as u64).to_be_bytes());
    calldata.extend_from_slice(&payload);
    let padding = (32 - (payload_len % 32)) % 32;
    calldata.extend_from_slice(&vec![0u8; padding]);

    // High `gas_price` to exercise the pre-fix native_per_pubdata overflow
    // path. With L1_TX_NATIVE_PRICE = 10 this drove native_per_gas to ~10^14
    // and native_per_pubdata to ~8·10^16, making the refund-time
    // `pubdata_used · native_per_pubdata` overflow u64.
    let high_gas_price = 10u128.pow(15);

    let tx = L1TxBuilder::new()
        .from(from)
        .to(l1_messenger)
        .gas_price(high_gas_price)
        .gas_limit(gas_limit.into())
        .gas_per_pubdata_byte_limit(gas_per_pubdata.into())
        .input(calldata)
        .build()
        .into();

    let mut tester = TestingFramework::new()
        .with_system_contracts(true, false)
        .with_balance(from, U256::MAX);

    let output = tester.execute_block(vec![tx]);
    let tx_result = output.tx_results[0]
        .as_ref()
        .expect("L1 tx must be processed");

    assert!(
        tx_result.is_success(),
        "L1 tx with large L2→L1 message must succeed; got: {:?}",
        output.tx_results[0]
    );

    // Each L1 message data byte ≈ one pubdata byte (plus a fixed L2ToL1Log
    // envelope), so the tx should report close to `payload_len` pubdata.
    assert!(
        tx_result.pubdata_used >= payload_len as u64,
        "expected at least {payload_len} pubdata bytes, got {}",
        tx_result.pubdata_used
    );
    // And we must stay within the gas-implied theoretical budget.
    assert!(
        tx_result.pubdata_used <= theoretical_budget,
        "pubdata_used ({}) exceeds the theoretical budget \
         ({theoretical_budget} = gas_limit / gas_per_pubdata)",
        tx_result.pubdata_used,
    );
}

/// Regression test: L1 transaction returndata must be cleared when the
/// post-execution pubdata check forces a revert.
///
/// When an L1 tx body executes successfully but the remaining gas cannot
/// cover the generated pubdata, the transaction is marked as failed.
/// Previously, the returndata from the successful execution leaked into
/// the revert output. This verifies the fix that clears returndata in
/// that case.
#[test]
fn test_l1_tx_returndata_cleared_on_pubdata_revert() {
    let from = address!("1234000000000000000000000000000000000000");
    let to = address!("0000000000000000000000000000000000010002");

    // Bytecode that writes to 10 storage slots (generating pubdata) and
    // returns 32 bytes containing the value 42.
    let mut builder = BytecodeBuilder::new().push_u8(42).push0().mstore();
    for slot in 0..10u8 {
        builder = builder.push_u8(1).push_u8(slot).sstore();
    }
    let bytecode = builder.push_u8(0x20).push0().return_().finish();

    // Control: L1 tx with cheap pubdata succeeds and returns non-empty data.
    let control_tx = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(10_000)
        .gas_limit(500_000)
        .gas_per_pubdata_byte_limit(1)
        .build()
        .into();

    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_balance(from, U256::from(1_000_000_000_000_000_u64));
    let control_output = tester.execute_block(vec![control_tx]);
    let control_result = control_output.tx_results[0]
        .as_ref()
        .expect("Control tx should be processed");
    assert!(
        control_result.is_success(),
        "Control tx should succeed, got: {:?}",
        control_output.tx_results[0]
    );
    match &control_result.execution_result {
        ExecutionResult::Success(ExecutionOutput::Call(output)) => {
            assert!(
                !output.is_empty(),
                "Control tx should return non-empty returndata"
            );
        }
        other => panic!("Unexpected control execution result: {other:?}"),
    }

    // Regression: same contract with expensive pubdata (high gas_per_pubdata)
    // so the post-execution pubdata check reverts the transaction.
    // The returndata from the successful execution must NOT leak through.
    let expensive_tx = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(10_000)
        .gas_limit(500_000)
        .gas_per_pubdata_byte_limit(1000)
        .build()
        .into();

    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_balance(from, U256::from(1_000_000_000_000_000_u64));
    let reverted_output = tester.execute_block(vec![expensive_tx]);
    let reverted_result = reverted_output.tx_results[0]
        .as_ref()
        .expect("Tx should be processed even if reverted by pubdata check");
    assert!(
        !reverted_result.is_success(),
        "Tx should be reverted by pubdata check, got: {:?}",
        reverted_output.tx_results[0]
    );
    match &reverted_result.execution_result {
        ExecutionResult::Revert(output) => {
            assert!(
                output.is_empty(),
                "Returndata must be cleared when L1 tx is reverted by pubdata check"
            );
        }
        other => panic!("Expected revert result, got: {other:?}"),
    }
}
