#![cfg(test)]

//!
//! Regression test for the B160 deserialization fix.
//!
//! This test tries to set as coinbase address a B160 with 24 bytes set instead of 20.
//!

use rig::alloy::consensus::TxEip2930;
use rig::alloy::primitives::{TxKind, U256};
use rig::ruint::aliases::B160;
use rig::{common_target_address, BlockContext, TestingFramework};
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

#[test]
#[should_panic]
fn test_invalid_coinbase() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let from = wallet.address();
    let target_address = common_target_address();

    tester = tester.with_balance(from, U256::from(1_000_000_000_000_000_u64));

    let tx = {
        let tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 0,
            gas_limit: 75_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    // Create invalid coinbase with 24 bytes set
    let mut coinbase = B160::ZERO;
    unsafe {
        for limb in coinbase.as_limbs_mut() {
            *limb = u64::MAX;
        }
    }

    let block_context = BlockContext {
        coinbase,
        eip1559_basefee: U256::ZERO,
        ..Default::default()
    };

    tester.set_block_context(Some(block_context));
    let _result = tester.execute_block(vec![tx]);
}
