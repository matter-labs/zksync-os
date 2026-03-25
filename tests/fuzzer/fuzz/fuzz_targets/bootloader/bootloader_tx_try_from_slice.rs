#![no_main]

use common::parse_abi_encoded_transaction;
use libfuzzer_sys::fuzz_target;

mod common;

fn fuzz(data: &[u8]) {
    let _ = parse_abi_encoded_transaction(data);
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
