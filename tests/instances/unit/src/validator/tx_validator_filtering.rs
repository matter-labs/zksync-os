#![cfg(test)]

//!
//! If TxValidator returns Err(FilteredByValidator) for a transaction,
//! that tx must NOT be included in the block
//! while other txs must still be included.
//!
//! Priority (L1 -> L2) transactions only go through the validator when the
//! chain opts in via `ChainConfig::l1_tx_filtering_enabled`, and a rejected one
//! is force-reverted instead of dropped: included, body discarded, full gas
//! limit charged, L1 result log with success = false. Upgrade transactions
//! never go through the validator.

use rig::alloy::consensus::TxLegacy;
use rig::alloy::primitives::{address, TxKind, B256};
use rig::chain::RunConfig;
use rig::forward_system::system::system_types::ForwardRunningSystem;
use rig::ruint::aliases::U256;
use rig::utils::L1TxBuilder;
use rig::zk_ee::system::tracer::NopTracer;
use rig::zk_ee::system::validator::{BeginTxContext, TxValidationError, TxValidator};
use rig::zksync_os_interface::error::InvalidTransaction;
use rig::zksync_os_interface::types::TxOutput;
use rig::{BlockOutput, TestingFramework};
use zksync_os_tests_common::zksync_tx::upgrade_tx::ZKsyncUpgradeTx;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

#[derive(Default)]
struct LoggingTxValidator {
    begin_calls: usize,
    finish_calls: usize,
    filter_on_begin: bool,
    filter_on_finish: bool,
}

impl LoggingTxValidator {
    fn new(filter_on_begin: bool, filter_on_finish: bool) -> Self {
        Self {
            begin_calls: 0,
            finish_calls: 0,
            filter_on_begin,
            filter_on_finish,
        }
    }
}

impl TxValidator<ForwardRunningSystem> for LoggingTxValidator {
    fn begin_tx(&mut self, _ctx: &BeginTxContext<'_>) -> Result<(), TxValidationError> {
        self.begin_calls += 1;
        println!("[TxValidator] begin_tx called (#{})", self.begin_calls);

        if self.filter_on_begin && self.begin_calls == 1 {
            println!("filtering tx in begin_tx");
            Err(TxValidationError::FilteredByValidator)
        } else {
            Ok(())
        }
    }

    fn finish_tx(&mut self) -> Result<(), TxValidationError> {
        self.finish_calls += 1;
        println!("[TxValidator] finish_tx called (#{})", self.finish_calls);

        if self.filter_on_finish && self.finish_calls == 1 {
            println!("[TxValidator] filtering tx in finish_tx");
            Err(TxValidationError::FilteredByValidator)
        } else {
            Ok(())
        }
    }
}

/// "tx_number_in_block" equivalent: index among included (successful) txs.
fn included_tx_number_in_block<T>(
    tx_results: &[Result<T, InvalidTransaction>],
    tx_index: usize,
) -> usize {
    tx_results[..tx_index].iter().filter(|r| r.is_ok()).count()
}

#[test]
fn test_tx_validator_filters_out_tx_without_bumping_counter() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let from = wallet.address();

    tester = tester.with_balance(from, U256::from(1_000_000_000_000_000_u64));

    let withdrawal_to = address!("000000000000000000000000000000000000800a");
    let withdrawal_calldata =
        hex::decode("51cff8d9000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();

    let mk_withdrawal = |nonce: u64, value: u64| {
        let tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(withdrawal_to),
            value: U256::from(value),
            input: withdrawal_calldata.clone().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    let tx0 = mk_withdrawal(0, 10);
    let tx1 = mk_withdrawal(0, 11);

    let mut tracer = NopTracer::default();
    let mut validator = LoggingTxValidator::new(true, false);

    // Disable RISC-V run because TxValidator is ignored during RISC-V execution
    tester.set_run_config(Some(RunConfig::without_riscv_run()));

    let out = tester.execute_block_with_tracing(vec![tx0, tx1], &mut tracer, &mut validator);

    println!(
        "[TxValidator] totals: begin_calls={}, finish_calls={}",
        validator.begin_calls, validator.finish_calls
    );

    // begin_tx is called for both transactions (1st is filtered, 2nd proceeds)
    assert_eq!(
        validator.begin_calls, 2,
        "begin_tx should be called for each tx"
    );

    // finish_tx is only called for the 2nd tx since the 1st was filtered by begin_tx
    // and never reaches process_l2_transaction where finish_tx is invoked
    assert_eq!(
        validator.finish_calls, 1,
        "finish_tx should only be called for txs that pass begin_tx"
    );

    // 1) First tx must be rejected
    assert!(
        matches!(
            out.tx_results[0],
            Err(InvalidTransaction::FilteredByValidator)
        ),
        "expected FilteredByValidator, got {:?}",
        out.tx_results[0]
    );

    // 2) Second tx must succeed
    assert!(out.tx_results[1].as_ref().is_ok_and(|o| o.is_success()));

    // 3) Second tx must be included as the first tx in block => number 0
    let tx1_number_in_block = included_tx_number_in_block(&out.tx_results, 1);
    assert_eq!(tx1_number_in_block, 0);
}

#[test]
fn test_no_custom_validator_does_not_restrict_tx_flow() {
    use rig::zk_ee::system::validator::NopTxValidator;

    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let from = wallet.address();

    tester = tester.with_balance(from, U256::from(1_000_000_000_000_000_u64));

    // Keep same tx shape; don't depend on L2->L1 logs.
    let withdrawal_to = address!("000000000000000000000000000000000000800a");
    let withdrawal_calldata =
        hex::decode("51cff8d9000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();

    let mk_withdrawal = |nonce: u64, value: u64| {
        let tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(withdrawal_to),
            value: U256::from(value),
            input: withdrawal_calldata.clone().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    // Normal nonces (0 then 1), because nothing is filtered.
    let tx0 = mk_withdrawal(0, 10);
    let tx1 = mk_withdrawal(1, 11);

    let mut tracer = NopTracer::default();
    let mut validator = NopTxValidator::default();

    let out = tester.execute_block_with_tracing(vec![tx0, tx1], &mut tracer, &mut validator);

    // 1) Both tx must succeed
    assert!(
        out.tx_results[0].as_ref().is_ok_and(|o| o.is_success()),
        "tx0 must succeed, got {:?}",
        out.tx_results[0]
    );
    assert!(
        out.tx_results[1].as_ref().is_ok_and(|o| o.is_success()),
        "tx1 must succeed, got {:?}",
        out.tx_results[1]
    );

    // 2) And tx_number_in_block must bump normally: first is 0, second is 1
    let tx0_number_in_block = included_tx_number_in_block(&out.tx_results, 0);
    assert_eq!(tx0_number_in_block, 0);

    let tx1_number_in_block = included_tx_number_in_block(&out.tx_results, 1);
    assert_eq!(tx1_number_in_block, 1);
}

#[test]
fn test_l1_transactions_are_not_filtered_by_validator() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let from = wallet.address();

    let withdrawal_to = address!("000000000000000000000000000000000000800a");
    let withdrawal_calldata =
        hex::decode("51cff8d9000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();

    tester = tester.with_balance(from, U256::from(10_000_000));

    let mk_l1_tx = |nonce: u64, value: u64| {
        L1TxBuilder::new()
            .from(from)
            .to(withdrawal_to)
            .gas_price(1000u128.into())
            .gas_limit(500_000u64.into())
            .value(U256::from(value))
            .input(withdrawal_calldata.clone().into())
            .nonce(nonce.into())
            .build()
            .into()
    };

    let tx0 = mk_l1_tx(0, 10);
    let tx1 = mk_l1_tx(1, 11);

    let mut tracer = NopTracer::default();
    let mut validator = LoggingTxValidator::new(true, false);

    let out = tester.execute_block_with_tracing(vec![tx0, tx1], &mut tracer, &mut validator);

    println!(
        "[TxValidator] totals: begin_calls={}, finish_calls={}",
        validator.begin_calls, validator.finish_calls
    );

    // L1 transactions should NOT be filtered by the validator
    // Validator.begin_tx() should never be called for L1 transactions
    assert_eq!(
        validator.begin_calls, 0,
        "L1 transactions should not trigger validator.begin_tx()"
    );

    // 1) Both L1 txs must succeed
    assert!(
        out.tx_results[0].as_ref().is_ok_and(|o| o.is_success()),
        "L1 tx0 should NOT be filtered by validator, got {:?}",
        out.tx_results[0]
    );
    assert!(
        out.tx_results[1].as_ref().is_ok_and(|o| o.is_success()),
        "L1 tx1 should NOT be filtered by validator, got {:?}",
        out.tx_results[1]
    );

    // 2) Both L1 txs should be included in block as txs 0 and 1
    let tx0_number_in_block = included_tx_number_in_block(&out.tx_results, 0);
    assert_eq!(tx0_number_in_block, 0);

    let tx1_number_in_block = included_tx_number_in_block(&out.tx_results, 1);
    assert_eq!(tx1_number_in_block, 1);
}

#[test]
fn test_tx_validator_filters_out_tx_on_begin_tx() {
    //! If a transaction is filtered by validator.begin_tx(),
    //! it should be rejected before execution and should not affect nonce counts.

    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let from = wallet.address();

    tester = tester.with_balance(from, U256::from(1_000_000_000_000_000_u64));

    let withdrawal_to = address!("000000000000000000000000000000000000800a");
    let withdrawal_calldata =
        hex::decode("51cff8d9000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();

    let mk_withdrawal = |nonce: u64, value: u64| {
        let tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(withdrawal_to),
            value: U256::from(value),
            input: withdrawal_calldata.clone().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    // Both txs with nonce 0 (first will be filtered, second should succeed with same nonce)
    let tx0 = mk_withdrawal(0, 10);
    let tx1 = mk_withdrawal(0, 11);

    let mut tracer = NopTracer::default();
    // Validator that filters on begin_tx only (first tx)
    let mut validator = LoggingTxValidator::new(true, false);

    // Disable RISC-V run because TxValidator is ignored during RISC-V execution
    tester.set_run_config(Some(RunConfig::without_riscv_run()));

    let out = tester.execute_block_with_tracing(vec![tx0, tx1], &mut tracer, &mut validator);

    println!(
        "[TxValidator] totals: begin_calls={}, finish_calls={}",
        validator.begin_calls, validator.finish_calls
    );

    // 1) First tx must be rejected by begin_tx
    assert!(
        matches!(
            out.tx_results[0],
            Err(InvalidTransaction::FilteredByValidator)
        ),
        "expected FilteredByValidator from begin_tx, got {:?}",
        out.tx_results[0]
    );

    // 2) Second tx must succeed even though both had nonce 0
    assert!(out.tx_results[1].as_ref().is_ok_and(|o| o.is_success()));

    // 3) Second tx must be included as the first tx in block => number 0
    let tx1_number_in_block = included_tx_number_in_block(&out.tx_results, 1);
    assert_eq!(tx1_number_in_block, 0);
}

/// Gas limit of the L1 test txs below. A force-reverted priority tx is charged all of it.
const L1_TX_GAS_LIMIT: u64 = 500_000;

/// Runs a block of two priority withdrawal txs (nonces 0 and 1) from a fresh
/// account on a chain with L1 tx filtering enabled, under `validator`.
///
/// Nothing is ever dropped from such a block, so the rig's forward vs
/// prover-input equivalence check runs: the recorded decisions are replayed
/// through the oracle validator.
fn run_l1_pair_with_filtering(
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) -> BlockOutput {
    let mut tester = TestingFramework::new().with_l1_tx_filtering(true);
    let from = tester.random_signer().address();
    tester = tester.with_balance(from, U256::from(10_000_000));

    let withdrawal_to = address!("000000000000000000000000000000000000800a");
    let withdrawal_calldata =
        hex::decode("51cff8d9000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();
    let mk_l1_tx = |nonce: u128, value: u64| {
        L1TxBuilder::new()
            .from(from)
            .to(withdrawal_to)
            .gas_price(1000)
            .gas_limit(L1_TX_GAS_LIMIT as u128)
            .value(U256::from(value))
            .input(withdrawal_calldata.clone())
            .nonce(nonce)
            .build()
    };

    tester.execute_block_with_tracing(
        vec![mk_l1_tx(0, 10), mk_l1_tx(1, 11)],
        &mut NopTracer::default(),
        validator,
    )
}

/// The priority tx at `idx` was force-reverted: included, failed, charged its
/// full gas limit, and its only L2 -> L1 log is the L1 tx result log with
/// `success = false` (the withdrawal message the body would send is gone).
fn assert_l1_tx_force_reverted(tx_results: &[Result<TxOutput, InvalidTransaction>], idx: usize) {
    let tx = tx_results[idx]
        .as_ref()
        .expect("force-reverted L1 tx must be included");
    assert!(!tx.is_success(), "force-reverted L1 tx {idx} must fail");
    assert_eq!(
        tx.gas_used, L1_TX_GAS_LIMIT,
        "L1 tx {idx} must be charged its full gas limit"
    );
    assert_eq!(
        tx.l2_to_l1_logs.len(),
        1,
        "L1 tx {idx} must only emit its result log"
    );
    assert_eq!(
        tx.l2_to_l1_logs[0].log.value,
        B256::ZERO,
        "L1 tx {idx} result log must be a failure"
    );
}

/// The priority tx at `idx` succeeded and its L1 tx result log says so.
fn assert_l1_tx_succeeded(tx_results: &[Result<TxOutput, InvalidTransaction>], idx: usize) {
    let tx = tx_results[idx].as_ref().expect("L1 tx must be included");
    assert!(tx.is_success(), "L1 tx {idx} should succeed, got {tx:?}");
    let result_log = tx
        .l2_to_l1_logs
        .last()
        .expect("successful L1 tx emits its result log");
    assert_eq!(
        result_log.log.value,
        B256::with_last_byte(1),
        "L1 tx {idx} result log must be a success"
    );
}

#[test]
fn test_l1_tx_rejected_before_execution_is_force_reverted() {
    // Rejects the first begin_tx call only.
    let mut validator = LoggingTxValidator::new(true, false);
    let out = run_l1_pair_with_filtering(&mut validator);

    // begin_tx for both txs; finish_tx only for tx1, whose body ran.
    assert_eq!((validator.begin_calls, validator.finish_calls), (2, 1));
    assert_l1_tx_force_reverted(&out.tx_results, 0);
    assert_l1_tx_succeeded(&out.tx_results, 1);
    // Both are included, in order.
    assert_eq!(included_tx_number_in_block(&out.tx_results, 1), 1);
}

#[test]
fn test_l1_tx_rejected_after_execution_is_force_reverted() {
    // Rejects the first finish_tx call only.
    let mut validator = LoggingTxValidator::new(false, true);
    let out = run_l1_pair_with_filtering(&mut validator);

    assert_eq!((validator.begin_calls, validator.finish_calls), (2, 2));
    assert_l1_tx_force_reverted(&out.tx_results, 0);
    assert_l1_tx_succeeded(&out.tx_results, 1);
    assert_eq!(included_tx_number_in_block(&out.tx_results, 1), 1);
}

#[test]
fn test_l1_tx_filtering_enabled_with_nop_validator_includes_all_l1_txs() {
    use rig::zk_ee::system::validator::NopTxValidator;

    let out = run_l1_pair_with_filtering(&mut NopTxValidator);

    assert_l1_tx_succeeded(&out.tx_results, 0);
    assert_l1_tx_succeeded(&out.tx_results, 1);
    assert_eq!(included_tx_number_in_block(&out.tx_results, 1), 1);
}

#[test]
fn test_upgrade_tx_is_not_filtered_when_l1_tx_filtering_enabled() {
    // A contract that always succeeds: PUSH1 0, PUSH1 0, RETURN.
    let contract_address = address!("0000000000000000000000000000000000010003");
    let mut tester = TestingFramework::new()
        .with_l1_tx_filtering(true)
        .with_evm_contract(contract_address, &hex::decode("60006000f3").unwrap());
    let upgrade_tx = ZKsyncTxEnvelope::from(ZKsyncUpgradeTx {
        from: address!("1234000000000000000000000000000000000000"),
        to: contract_address,
        gas_limit: 100_000u128,
        ..Default::default()
    });
    // Would reject the very first begin_tx call, if it were ever made.
    let mut validator = LoggingTxValidator::new(true, false);

    let out = tester.execute_block_with_tracing(
        vec![upgrade_tx],
        &mut NopTracer::default(),
        &mut validator,
    );

    assert_eq!((validator.begin_calls, validator.finish_calls), (0, 0));
    assert!(out.tx_results[0].as_ref().is_ok_and(|o| o.is_success()));
}
