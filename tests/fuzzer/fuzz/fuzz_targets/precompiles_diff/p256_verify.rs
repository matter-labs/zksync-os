#![no_main]
#![feature(allocator_api)]

use arbitrary::{Arbitrary,Unstructured};
use libfuzzer_sys::fuzz_target;
use revm_precompile::secp256r1::p256_verify;
use fuzz_precompiles_forward::precompiles::p256_verify as p256_forward;
use fuzz_precompiles_proving::precompiles::p256_verify as p256_proving;
use p256::ecdsa::{SigningKey,Signature};
use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::ecdsa::VerifyingKey;
use crate::common::{be_inc_inplace,be_dec_inplace};

mod common;

const IN_LEN: usize = 160;

const N_P256: [u8; 32] = [
    0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00,
    0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,
    0xbc,0xe6,0xfa,0xad,0xa7,0x17,0x9e,0x84,
    0xf3,0xb9,0xca,0xc2,0xfc,0x63,0x25,0x51,
];

const N_P256_HALF: [u8; 32] = [
    0x7f,0xff,0xff,0xff,0x80,0x00,0x00,0x00,
    0x7f,0xff,0xff,0xff,0xff,0xff,0xff,0xff,
    0xde,0x73,0x7d,0x56,0xd3,0x8b,0xcf,0x42,
    0x79,0xdc,0xe5,0x61,0x7e,0x31,0x92,0xa8,
];

#[derive(Arbitrary, Debug, Clone)]
struct Input {
    case: Case,
    mutation: Mutation,
    sk_seed: [u8; 32],
    msg: [u8; 32],
    flip_idx: u8,
    trunc: Option<u8>,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Case {
    Valid,          // Valid msg/pubkey/signature
    Mutate,         // see Mutation enum
    RandomBytes,    // Put random bytes
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Mutation {
    Msg,            // Bit flip mutate msg
    PubX,           // Bit flip mutate x coord of pubkey
    PubY,           // Bit flip mutate y coord of pubkey
    R,              // Bit flip mutate r part of signature
    S,              // Bit flip mutate s part of signature
    
    ZeroSig,        // r = 0, s = 0
    ZeroPub,        // (x,y) = (0,0)

    ZeroR,          // r = 0
    ZeroS,          // s = 0

    R_EqN,          // r = n
    S_EqN,          // s = n

    R_GeN,          // r = n + 1
    S_GeN,          // s = n + 1

    HighS,          // s = n - 1
    LowS,           // s = floor(n/2)
}

fn apply_mut(
    which: Mutation,
    msg: &mut [u8;32],
    px:  &mut [u8;32],
    py:  &mut [u8;32],
    r:   &mut [u8;32],
    s:   &mut [u8;32],
    flip_idx: u8,
) {
    let flip = |b: &mut [u8;32], at: u8| {
        let i = (at as usize) & 31;
        b[i] ^= 1;
    };

    match which {
        Mutation::Msg  => flip(msg, flip_idx),
        Mutation::PubX => flip(px, flip_idx),
        Mutation::PubY => flip(py, flip_idx),
        Mutation::R    => flip(r, flip_idx),
        Mutation::S    => flip(s, flip_idx),

        Mutation::ZeroSig => { *r = [0u8;32]; *s = [0u8;32]; }
        Mutation::ZeroPub => { *px = [0u8;32]; *py = [0u8;32]; }

        Mutation::ZeroR => { *r = [0u8;32]; }
        Mutation::ZeroS => { *s = [0u8;32]; }

        Mutation::R_EqN => { *r = N_P256; }
        Mutation::S_EqN => { *s = N_P256; }

        Mutation::R_GeN => { *r = N_P256; be_inc_inplace(r); }
        Mutation::S_GeN => { *s = N_P256; be_inc_inplace(s); }

        Mutation::HighS => { *s = N_P256; be_dec_inplace(s); }
        Mutation::LowS  => { *s = N_P256_HALF; },
    }
}

fn to_pub_xy(pk: &VerifyingKey) -> ([u8;32],[u8;32]) {
    let ep = pk.to_encoded_point(false);
    let x = ep.x().expect("X");
    let y = ep.y().expect("Y");
    let mut px = [0u8; 32];
    let mut py = [0u8; 32];
    px.copy_from_slice(x);
    py.copy_from_slice(y);
    (px, py)
}

fn valid_tuple(sk_seed: [u8; 32], msg: [u8; 32]) -> ( [u8;32],[u8;32],[u8;32],[u8;32],[u8;32] ) {
    let sk = SigningKey::from_bytes(&sk_seed.into()).unwrap_or_else(|_| {
        SigningKey::from_bytes(&[1u8;32].into()).unwrap()
    });
    let(px, py) = to_pub_xy(sk.verifying_key().into());

    let sig: Signature = sk.sign_prehash(&msg).expect("prehash sign");
    let sig_bytes = sig.to_bytes();
    let mut r = [0u8;32];
    let mut s = [0u8;32];
    r.copy_from_slice(&sig_bytes[..32]);
    s.copy_from_slice(&sig_bytes[32..]);
    (msg, px, py, r, s)
}

fn build_input(u: &mut Unstructured<'_>, i: &Input) -> Vec<u8> {
    match i.case {
        Case::RandomBytes => {
            let n = u.int_in_range::<usize>(0..=IN_LEN).unwrap_or(0);
            let mut v = vec![0u8; n];
            let _ = u.fill_buffer(&mut v);

            if let Some(t) = i.trunc {
                let t = t as usize;
                if t < v.len() { v.truncate(t); }
            }
            v
        }
        Case::Valid | Case::Mutate => {
            let (mut m, mut px, mut py, mut r, mut s) = valid_tuple(i.sk_seed, i.msg);
            if matches!(i.case, Case::Mutate) {
                apply_mut(i.mutation, &mut m, &mut px, &mut py, &mut r, &mut s, i.flip_idx);
            }

            let mut out = Vec::with_capacity(IN_LEN);
            out.extend_from_slice(&m);
            out.extend_from_slice(&r);
            out.extend_from_slice(&s);
            out.extend_from_slice(&px);
            out.extend_from_slice(&py);

            if let Some(t) = i.trunc {
                let t = t as usize;
                if t < out.len() { out.truncate(t); }
            }
            out
        }
    }
}

fn fuzz(data: &[u8]) {
    let mut u = Unstructured::new(data);
    let input = match Input::arbitrary(&mut u) {
        Ok(x) => x,
        Err(_) => return,
    };

    let in_bytes = build_input(&mut u, &input);

    let mut out_forward = Vec::new();
    let mut out_proving = Vec::new();

    let r_reth = p256_verify(in_bytes.as_slice(), u64::MAX);
    let reth_ok = r_reth.as_ref().is_ok_and(|x| !x.reverted);

    let r1 = p256_forward(in_bytes.as_slice(), &mut out_forward);
    let fwd_ok = r1.is_ok();

    let r2 = p256_proving(in_bytes.as_slice(), &mut out_proving);
    let prv_ok = r2.is_ok();

    match (reth_ok, fwd_ok, prv_ok) {
        (true, true, true) => {
            let reth_bytes = r_reth.unwrap().bytes.to_vec();
            assert_eq!(out_forward, reth_bytes, "forward <> reth bytes mismatch");
            assert_eq!(out_proving, reth_bytes, "proving <> reth bytes mismatch");
        }
        (false, false, false) => { }
        _ => {
            panic!(
                "status mismatch: reth_ok={reth_ok} fwd_ok={fwd_ok} prv_ok={prv_ok}, in_len={}",
                in_bytes.len()
            );
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});