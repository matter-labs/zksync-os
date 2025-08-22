#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use revm::precompile::modexp;
use rig::ethers::signers::Signer;
use ruint::aliases::{B160, U256};
mod common;

#[derive(Debug)]
struct Input {
    bsize: [u8; 32],
    esize: [u8; 32],
    msize: [u8; 32],
    b: Vec<u8>,
    e: Vec<u8>,
    m: Vec<u8>,
    n: usize,
}

impl Input {
    /// Concatenates all fields into a single `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let real_len = self.real_len();
        let fuzz_len = self.n;

        let mut result = Vec::new();

        // Append the 32-byte fields
        result.extend_from_slice(&self.bsize);
        result.extend_from_slice(&self.esize);
        result.extend_from_slice(&self.msize);

        // Append the variable-length fields
        result.extend_from_slice(&self.b);
        result.extend_from_slice(&self.e);
        result.extend_from_slice(&self.m);

        // Adjust the vector bytes according to the fuzzed length
        if fuzz_len > real_len {
            result.resize(fuzz_len, 0);
        } else {
            result.resize(real_len, 0);
        }

        result
    }

    pub fn real_len(&self) -> usize {
        self.bsize.len()
            + self.esize.len()
            + self.msize.len()
            + self.b.len()
            + self.e.len()
            + self.m.len()
    }
}

impl<'a> Arbitrary<'a> for Input {
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

        let real_len = b.len() + e.len() + m.len() + bsize.len() + esize.len() + msize.len();
        let n = u.int_in_range(0..=2 * real_len).unwrap();

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

fuzz_target!(|input: Input| {
    let mut chain = rig::Chain::empty(None);
    let wallet = chain.random_wallet();
    let tx = rig::utils::sign_and_encode_ethers_legacy_tx(
        common::get_tx(
            "0000000000000000000000000000000000000005",
            input.to_bytes().as_ref(),
        ),
        &wallet,
    );
    chain.set_balance(
        B160::from_be_bytes(wallet.address().0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let block_output = chain.run_block(vec![tx], None, None);

    #[allow(unused_variables)]
    let output = block_output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .expect("Tx should have succeeded");

    let zksync_os_bytes = output.as_returned_bytes();
    let bytes: alloy::primitives::Bytes = input.to_bytes().into();
    let revm_res = modexp::berlin_run(&bytes, 1 << 27);

    match revm_res {
        Ok(revm) => assert_eq!(zksync_os_bytes, revm.bytes.to_vec()),
        Err(_) => assert!(common::is_zero(zksync_os_bytes)),
    }
});
