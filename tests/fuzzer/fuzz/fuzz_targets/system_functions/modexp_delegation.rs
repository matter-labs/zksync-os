#![no_main]
#![feature(allocator_api)]

use arbitrary::{Arbitrary, Unstructured};
use basic_system::system_functions::modexp::delegation::delegated_modexp_with_naive_advisor;
use libfuzzer_sys::fuzz_target;
use ruint::aliases::U256;
use std::alloc::Global;
use std::convert::TryInto;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::reference_implementations::DecreasingNative;
use zk_ee::system::Resource;

#[derive(Debug)]
struct ModexpInput {
    bsize: [u8; 32],
    esize: [u8; 32],
    msize: [u8; 32],
    b: Vec<u8>,
    e: Vec<u8>,
    m: Vec<u8>,
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

        Ok(Self {
            bsize,
            esize,
            msize,
            b,
            e,
            m,
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

    let res_delegated = delegated_modexp_with_naive_advisor(&src.b, &src.e, &src.m);
    let res_forward = modexp::modexp(&src.b, &src.e, &src.m, Global);
    assert_eq!(
        normalize_be(&res_delegated),
        normalize_be(&res_forward),
        "mismatch for input = {src:#?}\ndelegated = {res_delegated:#?}\nforward = {res_forward:#?}"
    );
}

fn normalize_be(s: &[u8]) -> &[u8] {
    let i = s.iter().position(|&b| b != 0).unwrap_or(s.len());
    &s[i..]
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
