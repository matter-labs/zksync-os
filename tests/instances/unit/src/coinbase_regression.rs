#![cfg(test)]

//!
//! Regression test for the B160 deserialization fix.
//!
//! This test tries to set as coinbase address a B160 with 24 bytes set instead of 20.
//!

use rig::alloy::consensus::TxEip2930;
use rig::alloy::primitives::{address, TxKind, U256};
use rig::forward_system::run::convert_alloy::FromAlloy;
use rig::ruint::aliases::B160;
use rig::{BlockContext, Chain};
use zksync_os_tests_common::zksync_tx::encoding::ZKsyncOsEncodable;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

#[test]
#[should_panic]
fn test_invalid_coinbase() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    chain.set_balance(
        B160::from_alloy(from),
        U256::from(1_000_000_000_000_000_u64),
    );

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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    // Create invalid coinbase with 24 bytes set
    let mut coinbase = rig::ruint::aliases::B160::ZERO;
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

    let _result = chain.run_block(vec![tx], Some(block_context), None, None);
}
