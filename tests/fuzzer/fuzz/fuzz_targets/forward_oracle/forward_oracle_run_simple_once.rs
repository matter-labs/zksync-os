#![no_main]

use alloy::consensus::TxLegacy;
use alloy::primitives::{Bytes, TxKind, U256};
use libfuzzer_sys::fuzz_target;
use rig::ruint::aliases::B160;

fuzz_target!(|data: &[u8]| {
    let mut chain = rig::Chain::empty(None);

    let def_to = B160::from_str_radix("10002", 16).unwrap();

    let from = chain.random_signer();
    let to = if data.len() > 1 {
        B160::try_from_be_slice(&data[1..]).unwrap_or(def_to)
    } else {
        def_to
    };

    let gas = 57000;
    let call_value = U256::from(0);

    let tx = rig::utils::sign_and_encode_alloy_tx(
        TxLegacy {
            chain_id: None,
            nonce: 0,
            gas_price: 1000,
            gas_limit: gas,
            to: TxKind::Call(to.to_be_bytes().into()),
            value: call_value,
            input: Bytes::from(data.to_vec()),
        },
        &from,
    );

    chain.set_balance(
        B160::from_be_bytes(from.address().into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );
    chain.run_block(vec![tx], None, None, None);
});
