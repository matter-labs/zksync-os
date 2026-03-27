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
use rig::ruint::aliases::U256;
use rig::tx_succeeded;
use rig::utils::L1TxBuilder;
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
        .gas_price(1500)
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

/// Test that an L1 transaction with a gas price that would overflow the
/// native_per_gas calculation is processed gracefully.
///
/// The calculation is: native_per_gas = gas_price.div_ceil(L1_TX_NATIVE_PRICE)
/// where L1_TX_NATIVE_PRICE = 10. To overflow u64, gas_price needs to be
/// > u64::MAX * 10.
///
/// Prior to the resilience changes, this would fail with
/// InvalidTransaction::NativeResourcesAreTooExpensive. Now, u64::MAX is used
/// via saturating arithmetic.
#[test]
fn test_l1_tx_gas_price_overflow_native_per_gas() {
    let from = address!("1234000000000000000000000000000000000000");
    let to = common_target_address();

    // L1_TX_NATIVE_PRICE = 10
    // To overflow u64 in native_per_gas calculation: gas_price / 10 > u64::MAX
    // So gas_price > u64::MAX * 10
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
