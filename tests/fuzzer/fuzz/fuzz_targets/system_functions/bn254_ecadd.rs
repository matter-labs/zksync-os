#![no_main]
#![feature(allocator_api)]

use arbitrary::Unstructured;
use basic_system::system_functions::bn254_ecadd::Bn254AddImpl;
use libfuzzer_sys::fuzz_target;
use zk_ee::reference_implementations::{BaseComputationalResources, BaseResources};
use zk_ee::system::SystemFunction;

const BN254_ECADD_SRC_REQUIRED_LENGTH: usize = 128;
const BN254_ECADD_DST_MIN_LENGTH: usize = 64;

fn fuzz(data: &[u8]) {
    let u = &mut Unstructured::new(data);
    let src = u
        .arbitrary::<[u8; BN254_ECADD_SRC_REQUIRED_LENGTH]>()
        .unwrap();
    let dst: Vec<u8> = u.arbitrary::<Vec<u8>>().unwrap_or_default();
    if dst.len() < BN254_ECADD_DST_MIN_LENGTH {
        return;
    }
    let n = u
        .arbitrary::<u8>()
        .unwrap_or(BN254_ECADD_SRC_REQUIRED_LENGTH as u8) as usize;
    if n > BN254_ECADD_SRC_REQUIRED_LENGTH {
        return;
    }

    let allocator = std::alloc::Global;
    let mut resource = BaseResources {
        spendable: BaseComputationalResources { ergs: u64::MAX },
    };

    let mut dst = dst.clone();

    let _ = Bn254AddImpl::execute(&src.as_slice()[0..n], &mut dst, &mut resource, allocator);
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
