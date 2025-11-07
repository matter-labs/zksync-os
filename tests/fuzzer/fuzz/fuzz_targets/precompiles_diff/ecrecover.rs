#![no_main]
#![feature(allocator_api)]

use arbitrary::{Arbitrary,Unstructured};
use libfuzzer_sys::fuzz_target;
use revm_precompile::secp256k1::ec_recover_run;
use fuzz_precompiles_forward::precompiles::ecrecover as ecrecover_forward;
use fuzz_precompiles_proving::precompiles::ecrecover as ecrecover_proving;
use secp256k1::{ecdsa::RecoverableSignature,Message,Secp256k1,SecretKey};
use crate::common::{be_inc_inplace,be_dec_inplace};

mod common;

const IN_LEN: usize = 128;

const N_SECP256K1: [u8; 32] = [
    0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,
    0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFE,
    0xBA,0xAE,0xDC,0xE6,0xAF,0x48,0xA0,0x3B,
    0xBF,0xD2,0x5E,0x8C,0xD0,0x36,0x41,0x41,
];

const N_SECP256K1_HALF: [u8; 32] = [
    0x7F,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,
    0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,
    0x5D,0x57,0x6E,0x73,0x57,0xA4,0x50,0x1D,
    0xDF,0xE9,0x2F,0x46,0x68,0x1B,0x20,0xA0,
];

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Case {
    Valid,
    Mutate,
    RandomBytes,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Mutation {
    Msg,    // Bit flip mutate msg
    V,      // Bit flip mutate v part of signature
    R,      // Bit flip mutate r part of signature
    S,      // Bit flip mutate s part of signature

    V_0,      // v = 0
    V_1,      // v = 1
    V_29,     // v = 29
    V_27,     // force 27
    V_28,     // force 28

    ZeroR,    // r = 0
    ZeroS,    // s = 0
    R_EqN,    // r = n
    S_EqN,    // s = n
    R_GeN,    // r = n+1
    S_GeN,    // s = n+1
    HighS,    // s = n-1
    LowS,     // s = floor(n/2)
}

#[derive(Arbitrary, Debug, Clone)]
struct Input {
    case: Case,
    which: Mutation,
    sk_seed: [u8; 32],
    msg: [u8; 32],
    flip_idx: u8,
    trunc: Option<u8>,
}

fn valid_tuple(seed: [u8; 32], msg32: [u8; 32]) -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
    let secp = Secp256k1::signing_only();

    let sk = SecretKey::from_slice(&seed).unwrap_or_else(|_| {
        SecretKey::from_slice(&[1u8; 32]).expect("non-zero sk")
    });

    let msg = Message::from_slice(&msg32).expect("prehash");
    let recsig: RecoverableSignature = secp.sign_ecdsa_recoverable(&msg, &sk);

    let (recid, sig64) = recsig.serialize_compact();

    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&sig64[..32]);
    s.copy_from_slice(&sig64[32..]);

    let mut v = [0u8; 32];
    v[31] = 27 + recid as u8;

    (v, r, s, msg32)
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
            let (mut v, mut r, mut s, mut m) = valid_tuple(i.sk_seed, i.msg);
            if matches!(i.case, Case::Mutate) {
                apply_mut(i.which, &mut m, &mut v, &mut r, &mut s, i.flip_idx);
            }

            let mut out = Vec::with_capacity(IN_LEN);
            out.extend_from_slice(&m);
            out.extend_from_slice(&v);
            out.extend_from_slice(&r);
            out.extend_from_slice(&s);

            if let Some(t) = i.trunc {
                let t = t as usize;
                if t < out.len() { out.truncate(t); }
            }
            out
        }
    }
}

fn apply_mut(mutation: Mutation, msg: &mut [u8;32], v: &mut [u8;32], r: &mut [u8;32], s: &mut [u8;32], flip_idx: u8) {
    let flip = |b: &mut [u8;32], at: u8| {
        let i = (at as usize) & 31;
        b[i] ^= 1;
    };
    match mutation {
        Mutation::Msg => flip(msg, flip_idx),
        Mutation::V   => flip(v, flip_idx),
        Mutation::R   => flip(r, flip_idx),
        Mutation::S   => flip(s, flip_idx),

        Mutation::V_0  => { *v = [0u8;32]; }
        Mutation::V_1  => { *v = [0u8;32]; v[31] = 1; }
        Mutation::V_29 => { *v = [0u8;32]; v[31] = 29; }
        Mutation::V_27 => { *v = [0u8;32]; v[31] = 27; }
        Mutation::V_28 => { *v = [0u8;32]; v[31] = 28; }

        Mutation::ZeroR => { *r = [0u8;32]; }
        Mutation::ZeroS => { *s = [0u8;32]; }
        Mutation::R_EqN => { *r = N_SECP256K1; }
        Mutation::S_EqN => { *s = N_SECP256K1; }
        Mutation::R_GeN => { *r = N_SECP256K1; be_inc_inplace(r); }
        Mutation::S_GeN => { *s = N_SECP256K1; be_inc_inplace(s); }

        Mutation::HighS => { *s = N_SECP256K1; be_dec_inplace(s); }
        Mutation::LowS  => { *s = N_SECP256K1_HALF; },
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

    let r_reth = ec_recover_run(in_bytes.as_slice(), u64::MAX);
    let reth_ok = r_reth.as_ref().is_ok_and(|x| !x.reverted);

    let r1 = ecrecover_forward(in_bytes.as_slice(), &mut out_forward);
    let fwd_ok = r1.is_ok();

    let r2 = ecrecover_proving(in_bytes.as_slice(), &mut out_proving);
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