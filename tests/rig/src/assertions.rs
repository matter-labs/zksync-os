//! Domain-specific assertion macros for ZKsync OS integration tests.
//!
//! These macros wrap `BlockOutput` checks in readable, descriptive panics.

/// Assert that transaction at `$idx` succeeded (bootloader accepted AND EVM succeeded).
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

/// Assert that transaction at `$idx` was EVM-level reverted (accepted by bootloader, failed in EVM).
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

/// Assert that transaction at `$idx` did not complete successfully.
///
/// This matches the existing rig-level `tx_failed()` helper semantics and
/// treats both bootloader rejection and EVM reverts as failures.
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
            Ok(tx_out) if !tx_out.is_success() => {}
            Ok(tx_out) => panic!(
                "assert_tx_failed!(output, {idx}): expected tx failure but tx succeeded.\n  output: {tx_out:?}"
            ),
        }
    }};
}

/// Assert that transaction at `$idx` was rejected at the bootloader level.
#[macro_export]
macro_rules! assert_tx_rejected {
    ($output:expr, $idx:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let result = output
            .tx_results
            .get(idx)
            .unwrap_or_else(|| panic!("assert_tx_rejected: no tx at index {idx}"));
        match result {
            Err(_) => {}
            Ok(tx_out) => panic!(
                "assert_tx_rejected!(output, {idx}): expected bootloader rejection but tx was processed.\n  output: {tx_out:?}"
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

/// Assert that `gas_used` for transaction `$idx` is less than `$max`.
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
                let used = tx_out.gas_used;
                if used >= max {
                    panic!("assert_gas_used_lt!(output, {idx}, {max}): used {used} >= {max}");
                }
            }
            Err(e) => panic!(
                "assert_gas_used_lt!(output, {idx}, {max}): tx was rejected by bootloader.\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that `gas_used` for transaction `$idx` is greater than `$min`.
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
                let used = tx_out.gas_used;
                if used <= min {
                    panic!("assert_gas_used_gt!(output, {idx}, {min}): used {used} <= {min}");
                }
            }
            Err(e) => panic!(
                "assert_gas_used_gt!(output, {idx}, {min}): tx was rejected by bootloader.\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that `gas_used` for transaction `$idx` is within `[$min, $max)`.
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
                let used = tx_out.gas_used;
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

/// Assert that `computational_native_used` for transaction `$idx` is less than `$max`.
#[macro_export]
macro_rules! assert_computational_native_used_lt {
    ($output:expr, $idx:expr, $max:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let max: u64 = $max;
        let result = output.tx_results.get(idx).unwrap_or_else(|| {
            panic!("assert_computational_native_used_lt: no tx at index {idx}")
        });
        match result {
            Ok(tx_out) => {
                let used = tx_out.computational_native_used;
                if used >= max {
                    panic!(
                        "assert_computational_native_used_lt!(output, {idx}, {max}): used {used} >= {max}"
                    );
                }
            }
            Err(e) => panic!(
                "assert_computational_native_used_lt!(output, {idx}, {max}): tx was rejected by bootloader.\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that `computational_native_used` for transaction `$idx` is greater than `$min`.
#[macro_export]
macro_rules! assert_computational_native_used_gt {
    ($output:expr, $idx:expr, $min:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let min: u64 = $min;
        let result = output.tx_results.get(idx).unwrap_or_else(|| {
            panic!("assert_computational_native_used_gt: no tx at index {idx}")
        });
        match result {
            Ok(tx_out) => {
                let used = tx_out.computational_native_used;
                if used <= min {
                    panic!(
                        "assert_computational_native_used_gt!(output, {idx}, {min}): used {used} <= {min}"
                    );
                }
            }
            Err(e) => panic!(
                "assert_computational_native_used_gt!(output, {idx}, {min}): tx was rejected by bootloader.\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that `computational_native_used` for transaction `$idx` is within `[$min, $max)`.
#[macro_export]
macro_rules! assert_computational_native_used_between {
    ($output:expr, $idx:expr, $min:expr, $max:expr) => {{
        let output = &$output;
        let idx: usize = $idx;
        let min: u64 = $min;
        let max: u64 = $max;
        let result = output.tx_results.get(idx).unwrap_or_else(|| {
            panic!("assert_computational_native_used_between: no tx at index {idx}")
        });
        match result {
            Ok(tx_out) => {
                let used = tx_out.computational_native_used;
                if used < min || used >= max {
                    panic!(
                        "assert_computational_native_used_between!(output, {idx}, {min}, {max}): used {used} is not in [{min}, {max})"
                    );
                }
            }
            Err(e) => panic!(
                "assert_computational_native_used_between!(output, {idx}, {min}, {max}): tx was rejected by bootloader.\n  error: {e:?}"
            ),
        }
    }};
}

/// Assert that the block output contains a storage write:
/// `address + key => value`.
///
/// Expected types:
/// - `$address`: `alloy::primitives::Address`
/// - `$key`: `alloy::primitives::B256`
/// - `$value`: `alloy::primitives::B256`
#[macro_export]
macro_rules! assert_storage_written {
    ($output:expr, $address:expr, $key:expr, $value:expr) => {{
        let output = &$output;
        let expected_addr = $address;
        let expected_key = $key;
        let expected_val = $value;
        let found = output.storage_writes.iter().any(|w| {
            w.account == expected_addr && w.account_key == expected_key && w.value == expected_val
        });
        if !found {
            panic!(
                "assert_storage_written!: no storage write found for address {:?}, key {:?}, value {:?}",
                expected_addr, expected_key, expected_val
            );
        }
    }};
}

/// Assert that the block output contains at least one event log from `$address` with `$topic0`.
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

/// Assert that the block output does NOT contain any event log from `$address` with `$topic0`.
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
            panic!("assert_block_events_count!: expected {expected} events, got {actual}");
        }
    }};
}

/// Assert that `chain.get_account_properties(&addr).balance` equals `$expected_balance`.
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

#[cfg(test)]
mod tests {
    use crate::alloy::consensus::Header;
    use crate::alloy::primitives::{Address, Sealable, U256};
    use crate::zksync_os_interface::error::InvalidTransaction;
    use crate::zksync_os_interface::types::{
        BlockOutput, ExecutionOutput, ExecutionResult, TxOutput,
    };

    fn tx_output(
        execution_result: ExecutionResult,
        gas_used: u64,
        computational_native_used: u64,
    ) -> TxOutput {
        TxOutput {
            execution_result,
            gas_used,
            gas_refunded: 0,
            computational_native_used,
            native_used: computational_native_used,
            pubdata_used: 0,
            contract_address: None,
            logs: vec![],
            l2_to_l1_logs: vec![],
            storage_writes: vec![],
        }
    }

    fn block_with_result(result: Result<TxOutput, InvalidTransaction>) -> BlockOutput {
        BlockOutput {
            header: Header::default().seal_slow(),
            tx_results: vec![result],
            storage_writes: vec![],
            account_diffs: vec![],
            published_preimages: vec![],
            computational_native_used: 0,
        }
    }

    #[test]
    fn assert_tx_failed_accepts_reverted_and_rejected_transactions() {
        let reverted = block_with_result(Ok(tx_output(ExecutionResult::Revert(vec![]), 42, 7)));
        assert_tx_failed!(reverted, 0);

        let rejected = block_with_result(Err(InvalidTransaction::LackOfFundForMaxFee {
            fee: U256::from(2_u64),
            balance: U256::from(1_u64),
        }));
        assert_tx_failed!(rejected, 0);
    }

    #[test]
    fn assert_tx_rejected_rejects_evm_reverts() {
        let reverted = block_with_result(Ok(tx_output(ExecutionResult::Revert(vec![]), 42, 7)));
        let panic = std::panic::catch_unwind(|| assert_tx_rejected!(reverted, 0));
        assert!(
            panic.is_err(),
            "reverted tx must not satisfy assert_tx_rejected!"
        );
    }

    #[test]
    fn assert_gas_used_macros_check_gas_used_field() {
        let output = block_with_result(Ok(tx_output(
            ExecutionResult::Success(ExecutionOutput::Call(vec![])),
            120,
            5,
        )));

        assert_gas_used_lt!(output, 0, 121);
        assert_gas_used_gt!(output, 0, 119);
        assert_gas_used_between!(output, 0, 120, 121);

        let wrong_metric = std::panic::catch_unwind(|| assert_gas_used_lt!(output, 0, 100));
        assert!(
            wrong_metric.is_err(),
            "assert_gas_used_* must compare gas_used, not computational_native_used"
        );
    }

    #[test]
    fn assert_computational_native_used_macros_check_native_metric() {
        let output = block_with_result(Ok(tx_output(
            ExecutionResult::Success(ExecutionOutput::Call(vec![])),
            120,
            5,
        )));

        assert_computational_native_used_lt!(output, 0, 6);
        assert_computational_native_used_gt!(output, 0, 4);
        assert_computational_native_used_between!(output, 0, 5, 6);

        let wrong_metric =
            std::panic::catch_unwind(|| assert_computational_native_used_lt!(output, 0, 5));
        assert!(
            wrong_metric.is_err(),
            "assert_computational_native_used_* must compare computational_native_used"
        );
    }

    #[test]
    fn assert_account_macros_work_with_testing_framework_addresses() {
        let mut framework =
            crate::TestingFramework::new().with_balance(Address::ZERO, U256::from(7));

        assert_account_balance!(framework, Address::ZERO, U256::from(7_u64));
        assert_nonce!(framework, Address::ZERO, 0_u64);
    }
}
