#![no_main]
#![feature(allocator_api)]

use arbitrary::Unstructured;
use basic_system::system_functions::ecrecover::EcRecoverImpl;
use libfuzzer_sys::fuzz_target;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::reference_implementations::DecreasingNative;
use zk_ee::system::logger::NullLogger;
use zk_ee::system::Resource;
use zk_ee::system::SystemFunctionExt;

const ECRECOVER_SRC_REQUIRED_LENGTH: usize = 128;

struct DummyOracle;

impl zk_ee::oracle::IOOracle for DummyOracle {
    type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

    fn raw_query<
        'a,
        I: zk_ee::oracle::usize_serialization::UsizeSerializable
            + zk_ee::oracle::usize_serialization::UsizeDeserializable,
    >(
        &'a mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<Self::RawIterator<'a>, zk_ee::system::errors::internal::InternalError> {
        unreachable!("oracle should not be consulted on native targets");
    }
}

fn fuzz(data: &[u8]) {
    let u = &mut Unstructured::new(data);
    let src = u
        .arbitrary::<[u8; ECRECOVER_SRC_REQUIRED_LENGTH]>()
        .unwrap();
    let dst: Vec<u8> = u.arbitrary::<Vec<u8>>().unwrap_or_default();
    if dst.is_empty() {
        return;
    }
    let n = u
        .arbitrary::<u8>()
        .unwrap_or(ECRECOVER_SRC_REQUIRED_LENGTH as u8) as usize;
    if n > ECRECOVER_SRC_REQUIRED_LENGTH {
        return;
    }

    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

    let mut dst = dst.clone();

    let _ = EcRecoverImpl::<false>::execute(
        &src.as_slice()[0..n],
        &mut dst,
        &mut resource,
        &mut DummyOracle,
        &mut NullLogger,
        allocator,
    );
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
