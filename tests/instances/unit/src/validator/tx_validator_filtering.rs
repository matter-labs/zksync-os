#![cfg(test)]

//!
//! If TxValidator returns Err(FilteredByValidator) for a transaction,
//! that tx must NOT be included in the block
//! while other txs must still be included.

use rig::alloy::consensus::TxLegacy;
use rig::alloy::primitives::{address, TxKind};
use rig::chain::RunConfig;
use rig::forward_system::system::system_types::ForwardRunningSystem;
use rig::ruint::aliases::U256;
use rig::utils::L1TxBuilder;
use rig::zk_ee::system::tracer::NopTracer;
use rig::zk_ee::system::validator::{TxValidationError, TxValidator};
use rig::zksync_os_interface::error::InvalidTransaction;
use rig::TestingFramework;
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
    fn begin_tx(&mut self, _calldata: &[u8]) -> Result<(), TxValidationError> {
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
