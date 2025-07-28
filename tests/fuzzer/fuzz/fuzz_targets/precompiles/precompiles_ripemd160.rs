#![no_main]

use crypto::ripemd160::{Digest, Ripemd160};
use fuzzer::utils::helpers::left_pad_bytes;
use libfuzzer_sys::fuzz_target;
use rig::ethers::signers::Signer;
use ruint::aliases::{B160, U256};

mod common;

fuzz_target!(|data: &[u8]| {
    let mut chain = rig::Chain::empty(None);
    let wallet = chain.random_wallet();
    let tx = rig::utils::sign_and_encode_ethers_legacy_tx(
        common::get_tx("0000000000000000000000000000000000000003", data),
        &wallet,
    );
    chain.set_balance(
        B160::from_be_bytes(wallet.address().0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let block_output = chain.run_block(vec![tx], None, None);

    let output = block_output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .expect("Tx should have succeeded");

    assert_eq!(
        left_pad_bytes(Ripemd160::digest(data).as_slice(), 32),
        output.as_returned_bytes(),
        "Hashes should match"
    );
});
