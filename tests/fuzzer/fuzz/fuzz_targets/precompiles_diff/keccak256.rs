#![no_main]
#![feature(allocator_api)]

use arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use revm::primitives::{keccak256, B256};
use fuzz_precompiles_forward::precompiles::keccak256 as keccak256_forward;
use fuzz_precompiles_proving::precompiles::keccak256 as keccak256_proving;

const KECCAK256_SRC_REQUIRED_LENGTH: usize = 32;

fn fuzz(data: &[u8]) {
    let u = &mut Unstructured::new(data);
    let src = u.arbitrary::<[u8; KECCAK256_SRC_REQUIRED_LENGTH]>().unwrap();
    let n = u
        .arbitrary::<u8>()
        .unwrap_or(KECCAK256_SRC_REQUIRED_LENGTH as u8) as usize;
    if n > KECCAK256_SRC_REQUIRED_LENGTH {
        return;
    }

    let mut dst1 = Vec::new();
    let mut dst2 = Vec::new();

    let digest: B256 = keccak256(msg);
    let result_bytes: [u8; 32] = digest.to_fixed_bytes();

    let r1 = keccak256_forward(&src.as_slice()[0..n], &mut dst1);
    let r1_ok = r1.is_ok();

    let r2 = keccak256_proving(&src.as_slice()[0..n], &mut dst2);
    let r2_ok = r2.is_ok();

    if r1_ok || r2_ok {
        assert!(r1_ok,   "forward run rejected but proving run accepted");
        assert!(r2_ok,   "proving run rejected but forward run accepted");
        
        assert_eq!(dst1, result_bytes, "forward <> reth mismatch");
        assert_eq!(dst2, result_bytes, "proving <> reth mismatch");
    }
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
