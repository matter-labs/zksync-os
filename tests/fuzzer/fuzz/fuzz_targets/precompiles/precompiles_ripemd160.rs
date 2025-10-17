#![no_main]

use crypto::ripemd160::{Digest, Ripemd160};
use fuzzer::utils::helpers::left_pad_bytes;
use libfuzzer_sys::fuzz_target;

mod common;

fuzz_target!(|data: &[u8]| {
    let block_output = common::run_precompile("0000000000000000000000000000000000000003", data);

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
