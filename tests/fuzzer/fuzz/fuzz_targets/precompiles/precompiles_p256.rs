#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use revm::precompile;
mod common;

const P256_SRC_REQUIRED_LENGTH: usize = 160;

#[derive(Debug, Arbitrary)]
struct Input {
    src: [u8; P256_SRC_REQUIRED_LENGTH],
}

fuzz_target!(|input: Input| {
    let block_output = common::run_precompile(
        "0000000000000000000000000000000000000100",
        input.src.as_ref(),
    );

    #[allow(unused_variables)]
    let output = block_output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .expect("Tx should have succeeded");

    let zksync_os_bytes = output.as_returned_bytes();
    let bytes: alloy::primitives::Bytes = input.src.into();
    let revm_res = precompile::secp256r1::p256_verify(&bytes, 1 << 27);

    match revm_res {
        Ok(revm) => assert_eq!(zksync_os_bytes, revm.bytes.to_vec()),
        Err(_) => assert!(common::is_zero(zksync_os_bytes)),
    }
});
