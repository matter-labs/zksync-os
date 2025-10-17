#![no_main]

use libfuzzer_sys::fuzz_target;
mod common;

fuzz_target!(|data: &[u8]| {
    let block_output = common::run_precompile("0000000000000000000000000000000000000004", data);

    let output = block_output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .expect("Tx should have succeeded");

    assert_eq!(
        data,
        output.as_returned_bytes(),
        "Precompile ID output should be equal to the input"
    );
});
