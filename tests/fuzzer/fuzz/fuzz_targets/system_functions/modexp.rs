#![no_main]
#![feature(allocator_api)]

use arbitrary::{Arbitrary, Unstructured};
use basic_system::system_functions::modexp::ModExpImpl;
use libfuzzer_sys::fuzz_target;
use std::convert::TryInto;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::reference_implementations::DecreasingNative;
use zk_ee::system::Resource;
use zk_ee::system::SystemFunctionExt;

#[derive(Debug)]
struct ModexpInput {
    bsize: [u8; 32],
    esize: [u8; 32],
    msize: [u8; 32],
    b: Vec<u8>,
    e: Vec<u8>,
    m: Vec<u8>,
    n: usize,
}

impl ModexpInput {
    /// Concatenates all fields into a single `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();

        // Append the 32-byte fields
        result.extend_from_slice(&self.bsize);
        result.extend_from_slice(&self.esize);
        result.extend_from_slice(&self.msize);

        // Append the variable-length fields
        result.extend_from_slice(&self.b);
        result.extend_from_slice(&self.e);
        result.extend_from_slice(&self.m);

        result
    }
}

impl<'a> Arbitrary<'a> for ModexpInput {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut bsize_base = [0u8; 1];
        let mut bsize = [0u8; 32];
        u.fill_buffer(&mut bsize_base)?;
        bsize[31..32].copy_from_slice(&bsize_base);

        let mut esize_base = [0u8; 1];
        let mut esize = [0u8; 32];
        u.fill_buffer(&mut esize_base)?;
        esize[31..32].copy_from_slice(&esize_base);

        let mut msize_base = [0u8; 1];
        let mut msize = [0u8; 32];
        u.fill_buffer(&mut msize_base)?;
        msize[31..32].copy_from_slice(&msize_base);

        // Interpret the first byte as the lengths for b, e, and m
        let bsize_len = u8::from_be_bytes(bsize_base[..1].try_into().unwrap());
        let esize_len = u8::from_be_bytes(esize_base[..1].try_into().unwrap());
        let msize_len = u8::from_be_bytes(msize_base[..1].try_into().unwrap());

        let b = u.bytes(bsize_len as usize)?.to_vec();
        let e = u.bytes(esize_len as usize)?.to_vec();
        let m = u.bytes(msize_len as usize)?.to_vec();

        let n = b.len() + e.len() + m.len() + bsize.len() + esize.len() + msize.len();

        let n = u.int_in_range(0..=n).unwrap();

        Ok(Self {
            bsize,
            esize,
            msize,
            b,
            e,
            m,
            n,
        })
    }
}

fn fuzz(data: &[u8]) {
    let u = &mut Unstructured::new(data);
    let Ok(src) = u.arbitrary::<ModexpInput>() else {
        return;
    };
    let dst: Vec<u8> = u.arbitrary::<Vec<u8>>().unwrap_or_default();
    if dst.is_empty() {
        return;
    }

    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

    let mut dst = dst.clone();

    let _ = ModExpImpl::execute(
        &src.to_bytes().as_slice()[0..src.n],
        &mut dst,
        &mut resource,
        // We're in x86 target, so oracle and logger aren't going to be used.
        &mut DummyOracle {},
        &mut zk_ee::system::NullLogger,
        allocator,
    );
}

struct DummyOracle {}

impl zk_ee::oracle::IOOracle for DummyOracle {
    type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

    fn raw_query<'a, I: zk_ee::oracle::usize_serialization::UsizeSerializable + zk_ee::oracle::usize_serialization::UsizeDeserializable>(
        &'a mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<Self::RawIterator<'a>, zk_ee::system::errors::internal::InternalError> {
        unreachable!("oracle should not be consulted on native targets");
    }
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});