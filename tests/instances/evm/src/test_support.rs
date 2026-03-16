//! Shared helpers for domain-specific EVM execution tests.

use rig::alloy::consensus::TxEip1559;
use rig::alloy::primitives::{Address, TxKind, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::constants::{DEFAULT_MAX_FEE, DEFAULT_PRIORITY_FEE, TEST_CHAIN_ID};
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::TestingFramework;

pub(super) fn new_tester() -> TestingFramework<false> {
    TestingFramework::new()
}

pub(super) fn call_tx(signer: PrivateKeySigner, to: Address, gas_limit: u64) -> ZKsyncTxEnvelope {
    call_tx_with(
        signer,
        to,
        0,
        gas_limit,
        AlloyU256::ZERO,
        vec![],
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn call_tx_with(
    signer: PrivateKeySigner,
    to: Address,
    nonce: u64,
    gas_limit: u64,
    value: AlloyU256,
    calldata: Vec<u8>,
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
        input: calldata.into(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, signer)
}

pub(super) fn create_tx(
    signer: PrivateKeySigner,
    gas_limit: u64,
    init_code: Vec<u8>,
) -> ZKsyncTxEnvelope {
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
