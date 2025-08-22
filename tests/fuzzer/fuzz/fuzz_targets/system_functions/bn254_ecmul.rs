#![no_main]
#![feature(allocator_api)]

use arbitrary::Unstructured;
use basic_system::system_functions::bn254_ecmul::Bn254MulImpl;
use libfuzzer_sys::fuzz_target;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::system::SystemFunction;
use zk_ee::system::Resource;
use zk_ee::reference_implementations::DecreasingNative;

const BN254_ECMUL_SRC_REQUIRED_LENGTH: usize = 96;
const BN254_ECMUL_DST_MIN_LENGTH: usize = 64;

fn fuzz(data: &[u8]) {
    let u = &mut Unstructured::new(data);
    let src = u
        .arbitrary::<[u8; BN254_ECMUL_SRC_REQUIRED_LENGTH]>()
        .unwrap();
    let dst: Vec<u8> = u.arbitrary::<Vec<u8>>().unwrap_or_default();
    if dst.len() < BN254_ECMUL_DST_MIN_LENGTH {
        return;
    }
    let n = u
        .arbitrary::<u8>()
        .unwrap_or(BN254_ECMUL_SRC_REQUIRED_LENGTH as u8) as usize;
    if n > BN254_ECMUL_SRC_REQUIRED_LENGTH {
        return;
    }

    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

    let mut dst = dst.clone();

    let _ = Bn254MulImpl::execute(&src.as_slice()[0..n], &mut dst, &mut resource, allocator);
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
