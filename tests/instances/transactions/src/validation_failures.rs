//! Transaction validation failures and other pre-execution rejection paths.

use rig::alloy::consensus::TxEip1559;
use rig::alloy::primitives::{address, Address, TxKind, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::constants::*;
use rig::ruint::aliases::U256;
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::TestingFramework;
use rig::{assert_tx_rejected, assert_tx_success};

fn new_tester() -> TestingFramework<false> {
    TestingFramework::new()
}

#[allow(clippy::too_many_arguments)]
fn call_tx(
    signer: PrivateKeySigner,
    to: Address,
    nonce: u64,
    gas_limit: u64,
    value: AlloyU256,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
    chain_id: u64,
) -> ZKsyncTxEnvelope {
    let tx = TxEip1559 {
        chain_id,
        nonce,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        gas_limit,
        to: TxKind::Call(to),
        value,
        access_list: Default::default(),
        input: Default::default(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, signer)
}

fn create_tx(signer: PrivateKeySigner, gas_limit: u64, init_code: Vec<u8>) -> ZKsyncTxEnvelope {
    let tx = TxEip1559 {
        chain_id: TEST_CHAIN_ID,
        nonce: 0,
        max_fee_per_gas: DEFAULT_MAX_FEE,
        max_priority_fee_per_gas: DEFAULT_PRIORITY_FEE,
        gas_limit,
        to: TxKind::Create,
        value: AlloyU256::ZERO,
        access_list: Default::default(),
        input: init_code.into(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, signer)
}

#[test]
fn out_of_gas_simple_transfer_is_rejected_during_validation() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("deadbeef00000000000000000000000000000001");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));
    let tx = call_tx(
        signer,
        recipient,
        0,
        1,
        AlloyU256::ZERO,
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    );
    let output = tester.execute_block(vec![tx]);
    assert_tx_rejected!(output, 0);
}

#[test]
fn out_of_gas_deployment_is_rejected_during_validation() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let deploy_bytecode = hex::decode(rig::utils::ERC_20_DEPLOYMENT_BYTECODE).unwrap();

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));
    let tx = create_tx(signer, 5_000, deploy_bytecode);
    let output = tester.execute_block(vec![tx]);
    assert_tx_rejected!(output, 0);
}

#[test]
fn wrong_chain_id_is_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));
    let tx = call_tx(
        signer,
        recipient,
        0,
        TRANSFER_GAS_LIMIT,
        AlloyU256::ZERO,
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        1,
    );
    let output = tester.execute_block(vec![tx]);
    assert_tx_rejected!(output, 0);
}

#[test]
fn nonce_too_low_is_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx0 = call_tx(
        signer.clone(),
        recipient,
        0,
        TRANSFER_GAS_LIMIT,
        AlloyU256::ZERO,
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    );
    let out0 = tester.execute_block(vec![tx0]);
    assert_tx_success!(out0, 0);

    let tx_low = call_tx(
        signer,
        recipient,
        0,
        TRANSFER_GAS_LIMIT,
        AlloyU256::ZERO,
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    );
    let out1 = tester.execute_block(vec![tx_low]);
    assert_tx_rejected!(out1, 0);
}

#[test]
fn nonce_too_high_is_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));
    let tx = call_tx(
        signer,
        recipient,
        5,
        TRANSFER_GAS_LIMIT,
        AlloyU256::ZERO,
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    );
    let output = tester.execute_block(vec![tx]);
    assert_tx_rejected!(output, 0);
}

#[test]
fn insufficient_balance_for_gas_is_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(1u64));
    let tx = call_tx(
        signer,
        recipient,
        0,
        TRANSFER_GAS_LIMIT,
        AlloyU256::from(1u64),
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    );
    let output = tester.execute_block(vec![tx]);
    assert_tx_rejected!(output, 0);
}

#[test]
fn max_fee_below_basefee_is_rejected() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let recipient = address!("0000000000000000000000000000000000000002");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));
    let tx = call_tx(
        signer,
        recipient,
        0,
        TRANSFER_GAS_LIMIT,
        AlloyU256::ZERO,
        1,
        0,
        TEST_CHAIN_ID,
    );
    let output = tester.execute_block(vec![tx]);
    assert_tx_rejected!(output, 0);
}
