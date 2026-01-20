#![cfg(test)]

//!
//! If TxValidator returns Err(FilteredByValidator) for a transaction,
//! that tx must NOT be included in the block (i.e. it must not bump tx_number_in_block),
//! while other txs must still be included.

use rig::alloy::consensus::TxLegacy;
use rig::alloy::primitives::{address, TxKind};
use rig::forward_system::system::system_types::ForwardRunningSystem;
use rig::ruint::aliases::{B160, U256};
use rig::zk_ee::system::tracer::NopTracer;
use rig::zk_ee::system::validator::{TxValidationError, TxValidator};
use rig::zksync_os_interface::error::InvalidTransaction;
use rig::Chain;

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

#[test]
fn test_tx_validator_filters_out_tx_without_bumping_counter() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();

    chain.set_balance(
        B160::from_be_bytes(from.into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );

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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let tx0 = mk_withdrawal(0, 10);
    let tx1 = mk_withdrawal(0, 11);

    let mut tracer = NopTracer::default();

    let mut validator = LoggingTxValidator::new(true, true);

    let result = chain.run_block_with_extra_stats(
        vec![tx0, tx1],
        None,
        None,
        None,
        &mut tracer,
        &mut validator,
    );

    assert!(result.is_ok());
    let (out, _, _) = result.unwrap();

    println!(
        "[TxValidator] totals: begin_calls={}, finish_calls={}",
        validator.begin_calls, validator.finish_calls
    );
    assert!(
        validator.finish_calls >= 2,
        "finish_tx should be called per tx"
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

    // 3) Second tx must be tx_number_in_block == 0
    let second = out.tx_results[1].clone().unwrap();
    let first_log = second
        .l2_to_l1_logs
        .first()
        .expect("withdrawal should emit L2->L1 log");

    assert_eq!(first_log.log.tx_number_in_block, 0);
}

#[test]
fn test_no_custom_validator_does_not_restrict_tx_flow() {
    use rig::zk_ee::system::validator::NopTxValidator;

    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();

    chain.set_balance(
        B160::from_be_bytes(from.into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );

    // L2 base token address for withdrawals
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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    // Here we use "normal" nonces (0 then 1), because nothing is filtered.
    let tx0 = mk_withdrawal(0, 10);
    let tx1 = mk_withdrawal(1, 11);

    let mut tracer = NopTracer::default();
    let mut validator = NopTxValidator::default();

    let result = chain.run_block_with_extra_stats(
        vec![tx0, tx1],
        None,
        None,
        None,
        &mut tracer,
        &mut validator,
    );

    assert!(result.is_ok());
    let (out, _, _) = result.unwrap();

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
    let first = out.tx_results[0].clone().unwrap();
    let first_log = first
        .l2_to_l1_logs
        .first()
        .expect("withdrawal should emit L2->L1 log");
    assert_eq!(first_log.log.tx_number_in_block, 0);

    let second = out.tx_results[1].clone().unwrap();
    let second_log = second
        .l2_to_l1_logs
        .first()
        .expect("withdrawal should emit L2->L1 log");
    assert_eq!(second_log.log.tx_number_in_block, 1);
}
