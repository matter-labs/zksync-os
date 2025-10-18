#![no_main]

use libfuzzer_sys::fuzz_target;
use revm::precompile::bn128;

mod common;

fuzz_target!(|data: &[u8]| {
    let block_output = common::run_precompile("0000000000000000000000000000000000000008", data);

    #[allow(unused_variables)]
    let output = block_output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .expect("Tx should have succeeded");

    let zksync_os_bytes = output.as_returned_bytes();
    let revm_res = bn128::run_pair(data, 0, 0, 1 << 27);

    match revm_res {
        Ok(revm) => assert_eq!(zksync_os_bytes, revm.bytes.to_vec()),
        Err(_) => assert!(common::is_zero(zksync_os_bytes)),
    }
});
