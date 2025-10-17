#![no_main]

use libfuzzer_sys::fuzz_target;
use sha2::{Digest, Sha256};

mod common;

fuzz_target!(|data: &[u8]| {
    let block_output = common::run_precompile("0000000000000000000000000000000000000002", data);

    let output = block_output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .expect("Tx should have succeeded");

    assert_eq!(
        Sha256::digest(data).as_slice(),
        output.as_returned_bytes(),
        "Hashes should match"
    );
});
