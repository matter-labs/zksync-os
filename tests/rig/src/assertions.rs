//! Domain-specific assertion macros for ZKsync OS integration tests.
//!
//! These macros wrap the raw `BlockOutput` in readable, descriptive panics.
//!
//! # Usage
//!
//! ```rust,ignore
//! use rig::assertions::*;  // or just rely on the re-export from `rig`
//!
//! let output = chain.run_block(txs, None, None, Some(run_config::full_proof()));
//!
//! assert_tx_success!(output, 0);          // transaction 0 succeeded
//! assert_tx_reverted!(output, 1);         // transaction 1 reverted (EVM-level)
//! assert_all_success!(output);            // every transaction succeeded
//! assert_gas_used_lt!(output, 0, 50_000); // tx 0 used < 50 000 gas
//! ```

/// Assert that transaction at `$idx` succeeded (bootloader accepted AND EVM succeeded).
///
/// Prints a human-readable diagnostic when the assertion fails.
#[macro_export]
macro_rules! assert_tx_success {
    ($output:expr, $idx:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let result = output
            .tx_results
            .get(idx)
            .unwrap_or_else(|| panic!("assert_tx_success: no tx at index {idx}"));
        match result {
            Ok(tx_out) if tx_out.is_success() => {}
            Ok(tx_out) => panic!(
                "assert_tx_success!(output, {idx}): tx was accepted but EVM reverted.\n  output: {tx_out:?}"
            ),
            Err(e) => panic!(
                "assert_tx_success!(output, {idx}): tx was rejected by bootloader.\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that transaction at `$idx` was EVM-level reverted (bootloader accepted, but EVM reverted).
#[macro_export]
macro_rules! assert_tx_reverted {
    ($output:expr, $idx:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let result = output
            .tx_results
            .get(idx)
            .unwrap_or_else(|| panic!("assert_tx_reverted: no tx at index {idx}"));
        match result {
            Ok(tx_out) if !tx_out.is_success() => {}
            Ok(tx_out) => panic!(
                "assert_tx_reverted!(output, {idx}): expected EVM revert but tx succeeded.\n  output: {tx_out:?}"
            ),
            Err(e) => panic!(
                "assert_tx_reverted!(output, {idx}): tx was rejected by bootloader (not an EVM revert).\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that transaction at `$idx` was rejected at the bootloader level (e.g. invalid nonce,
/// insufficient balance for gas, bad signature).
#[macro_export]
macro_rules! assert_tx_failed {
    ($output:expr, $idx:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let result = output
            .tx_results
            .get(idx)
            .unwrap_or_else(|| panic!("assert_tx_failed: no tx at index {idx}"));
        match result {
            Err(_) => {}
            Ok(tx_out) => panic!(
                "assert_tx_failed!(output, {idx}): expected bootloader rejection but tx was processed.\n  output: {tx_out:?}"
            ),
        }
    }};
}

/// Assert that every transaction in `$output` succeeded.
#[macro_export]
macro_rules! assert_all_success {
    ($output:expr) => {{
        let output = &$output;
        for (idx, result) in output.tx_results.iter().enumerate() {
            match result {
                Ok(tx_out) if tx_out.is_success() => {}
                Ok(tx_out) => panic!(
                    "assert_all_success!: tx {idx} was accepted but EVM reverted.\n  output: {tx_out:?}"
                ),
                Err(e) => panic!(
                    "assert_all_success!: tx {idx} was rejected by bootloader.\n  error: {e:?}"
                ),
            }
        }
    }};
}

/// Assert that `computational_native_used` for transaction `$idx` is less than `$max`.
#[macro_export]
macro_rules! assert_gas_used_lt {
    ($output:expr, $idx:expr, $max:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let max: u64 = $max;
        let result = output
            .tx_results
            .get(idx)
            .unwrap_or_else(|| panic!("assert_gas_used_lt: no tx at index {idx}"));
        match result {
            Ok(tx_out) => {
                let used = tx_out.computational_native_used;
                if used >= max {
                    panic!(
                        "assert_gas_used_lt!(output, {idx}, {max}): used {used} >= {max}"
                    );
                }
            }
            Err(e) => panic!(
                "assert_gas_used_lt!(output, {idx}, {max}): tx was rejected by bootloader.\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that the block output contains a storage write to the given `account` address at
/// `account_key` with the expected `value`.
///
/// Compares `StorageWrite::account`, `StorageWrite::account_key`, and `StorageWrite::value`
/// fields (all as `[u8; 32]` big-endian byte arrays).
///
/// # Arguments
/// - `$output` — the `BlockOutput` from `chain.run_block(...)`
/// - `$address_bytes` — `[u8; 32]` left-padded address (use `addr.to_be_bytes::<32>()` for a B160)
/// - `$key_bytes` — `[u8; 32]` slot key (use `key.to_be_bytes::<32>()` for a U256)
/// - `$value_bytes` — `[u8; 32]` expected value
#[macro_export]
macro_rules! assert_storage_written {
    ($output:expr, $address_bytes:expr, $key_bytes:expr, $value_bytes:expr) => {{
        let output = &$output;
        let expected_addr: [u8; 32] = $address_bytes;
        let expected_key: [u8; 32] = $key_bytes;
        let expected_val: [u8; 32] = $value_bytes;
        let found = output.storage_writes.iter().any(|w| {
            let acc: [u8; 32] = w.account.0;
            let akey: [u8; 32] = w.account_key.0;
            let val: [u8; 32] = w.value.0;
            acc == expected_addr && akey == expected_key && val == expected_val
        });
        if !found {
            panic!(
                "assert_storage_written!: no storage write found for address {:02x?}, key {:02x?}, value {:02x?}",
                expected_addr, expected_key, expected_val
            );
        }
    }};
}

/// Assert that `computational_native_used` for transaction `$idx` is greater than `$min`.
///
/// Useful for regression tests: "gas used must be at least X — not optimized away".
#[macro_export]
macro_rules! assert_gas_used_gt {
    ($output:expr, $idx:expr, $min:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let min: u64 = $min;
        let result = output
            .tx_results
            .get(idx)
            .unwrap_or_else(|| panic!("assert_gas_used_gt: no tx at index {idx}"));
        match result {
            Ok(tx_out) => {
                let used = tx_out.computational_native_used;
                if used <= min {
                    panic!(
                        "assert_gas_used_gt!(output, {idx}, {min}): used {used} <= {min}"
                    );
                }
            }
            Err(e) => panic!(
                "assert_gas_used_gt!(output, {idx}, {min}): tx was rejected by bootloader.\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that `computational_native_used` for transaction `$idx` is within `[$min, $max)`.
///
/// Useful for regression guard tests: "gas stays in a known window after a refactor".
#[macro_export]
macro_rules! assert_gas_used_between {
    ($output:expr, $idx:expr, $min:expr, $max:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let min: u64 = $min;
        let max: u64 = $max;
        let result = output
            .tx_results
            .get(idx)
            .unwrap_or_else(|| panic!("assert_gas_used_between: no tx at index {idx}"));
        match result {
            Ok(tx_out) => {
                let used = tx_out.computational_native_used;
                if used < min || used >= max {
                    panic!(
                        "assert_gas_used_between!(output, {idx}, {min}, {max}): used {used} is not in [{min}, {max})"
                    );
                }
            }
            Err(e) => panic!(
                "assert_gas_used_between!(output, {idx}, {min}, {max}): tx was rejected by bootloader.\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that the block output contains at least one event log from `$address` with the given
/// `$topic0` as the first topic (across all transactions in the block).
///
/// Iterates over all successful transactions' `logs` fields.
///
/// # Arguments
/// - `$output` — the `BlockOutput` from `chain.run_block(...)`
/// - `$address` — `alloy::primitives::Address` emitter address
/// - `$topic0` — `alloy::primitives::B256` expected first topic (event signature hash)
#[macro_export]
macro_rules! assert_event_emitted {
    ($output:expr, $address:expr, $topic0:expr) => {{
        let output = &$output;
        let expected_addr = $address;
        let expected_topic0 = $topic0;
        let found = output.tx_results.iter().any(|r| {
            r.as_ref()
                .ok()
                .map(|tx_out| {
                    tx_out.logs.iter().any(|ev| {
                        ev.address == expected_addr
                            && ev
                                .topics()
                                .first()
                                .map(|t| *t == expected_topic0)
                                .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        });
        if !found {
            panic!(
                "assert_event_emitted!: no event found from address {:?} with topic0 {:?}",
                expected_addr, expected_topic0
            );
        }
    }};
}

/// Assert that the block output does NOT contain any event log from `$address` with the given
/// `$topic0` as the first topic.
///
/// Useful for confirming that a reverted call did not produce system events.
#[macro_export]
macro_rules! assert_event_not_emitted {
    ($output:expr, $address:expr, $topic0:expr) => {{
        let output = &$output;
        let expected_addr = $address;
        let expected_topic0 = $topic0;
        let found = output.tx_results.iter().any(|r| {
            r.as_ref()
                .ok()
                .map(|tx_out| {
                    tx_out.logs.iter().any(|ev| {
                        ev.address == expected_addr
                            && ev
                                .topics()
                                .first()
                                .map(|t| *t == expected_topic0)
                                .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        });
        if found {
            panic!(
                "assert_event_not_emitted!: unexpected event from address {:?} with topic0 {:?}",
                expected_addr, expected_topic0
            );
        }
    }};
}

/// Assert the total number of event logs emitted in the block equals `$expected_count`.
///
/// Counts logs across all successful transactions' `logs` fields.
/// Useful for regression tests on system hook event emission.
#[macro_export]
macro_rules! assert_block_events_count {
    ($output:expr, $expected_count:expr) => {{
        let output = &$output;
        let expected: usize = $expected_count;
        let actual: usize = output
            .tx_results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .map(|tx_out| tx_out.logs.len())
            .sum();
        if actual != expected {
            panic!(
                "assert_block_events_count!: expected {expected} events, got {actual}"
            );
        }
    }};
}

/// Assert that `chain.get_account_properties(&addr).balance` equals `$expected_balance`.
///
/// Call this after a block run to verify that an account's balance has been updated correctly.
///
/// # Arguments
/// - `$chain` — mutable reference to the [`Chain`]
/// - `$addr` — `ruint::aliases::B160` address
/// - `$expected_balance` — `ruint::aliases::U256` expected balance
#[macro_export]
macro_rules! assert_account_balance {
    ($chain:expr, $addr:expr, $expected_balance:expr) => {{
        let chain = &mut $chain;
        let addr = $addr;
        let expected = $expected_balance;
        let actual = chain.get_account_properties(&addr).balance;
        if actual != expected {
            panic!(
                "assert_account_balance!: address {:?} has balance {actual}, expected {expected}",
                addr
            );
        }
    }};
}

/// Assert that `chain.get_account_properties(&addr).nonce` equals `$expected_nonce`.
///
/// # Arguments
/// - `$chain` — mutable reference to the [`Chain`]
/// - `$addr` — `ruint::aliases::B160` address
/// - `$expected_nonce` — `u64` expected nonce
#[macro_export]
macro_rules! assert_nonce {
    ($chain:expr, $addr:expr, $expected_nonce:expr) => {{
        let chain = &mut $chain;
        let addr = $addr;
        let expected: u64 = $expected_nonce;
        let actual = chain.get_account_properties(&addr).nonce;
        if actual != expected {
            panic!(
                "assert_nonce!: address {:?} has nonce {actual}, expected {expected}",
                addr
            );
        }
    }};
}
