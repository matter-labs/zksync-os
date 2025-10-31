#![no_main]
#![feature(allocator_api)]

use arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use fuzz_precompiles_forward::precompiles::ripemd160 as ripemd160_forward;
use fuzz_precompiles_proving::precompiles::ripemd160 as ripemd160_proving;
use revm_precompile::hash;

const RIPEMD160_SRC_REQUIRED_LENGTH: usize = 32;

fn fuzz(data: &[u8]) {
    let u = &mut Unstructured::new(data);
    let src = u.arbitrary::<[u8; RIPEMD160_SRC_REQUIRED_LENGTH]>().unwrap();
    let n = u
        .arbitrary::<u8>()
        .unwrap_or(RIPEMD160_SRC_REQUIRED_LENGTH as u8) as usize;
    if n > RIPEMD160_SRC_REQUIRED_LENGTH {
        return;
    }

    let mut dst1 = Vec::new();
    let mut dst2 = Vec::new();

    let r_reth = hash::ripemd160_run(&src.as_slice()[0..n], u64::MAX);
    let reth_ok = r_reth.as_ref().is_ok_and(|x| !x.reverted);

    let r1 = ripemd160_forward(&src.as_slice()[0..n], &mut dst1);
    let r1_ok = r1.is_ok();

    let r2 = ripemd160_proving(&src.as_slice()[0..n], &mut dst2);
    let r2_ok = r2.is_ok();

    if reth_ok || r1_ok || r2_ok {
        assert!(reth_ok,  "reth reverted but others accepted");
        assert!(r1_ok,   "forward run rejected but reth accepted");
        assert!(r2_ok,   "proving run rejected but reth accepted");

        let reth_out = r_reth.unwrap().bytes.to_vec();
        assert_eq!(dst1, reth_out, "forward <> reth bytes mismatch");
        assert_eq!(dst2, reth_out, "proving <> reth bytes mismatch");
    }
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
