#![no_main]
#![feature(allocator_api)]

use arbitrary::{Arbitrary,Unstructured};
use libfuzzer_sys::fuzz_target;
use ark_bn254::{Fq,Fq2,G1Affine,G2Affine,G1Projective,G2Projective};
use ark_ec::{CurveGroup,Group,AffineRepr};
use ark_ff::{BigInteger,PrimeField};
use revm_precompile::bn254::run_pair;
use fuzz_precompiles_forward::precompiles::pairing as pairing_forward;
use fuzz_precompiles_proving::precompiles::pairing as pairing_proving;

const CHUNK: usize = 32;
const G1_SIZE: usize = 64;
const G2_SIZE: usize = 128;
const PAIR_SIZE: usize = G1_SIZE + G2_SIZE;

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Coord {
    G1x,
    G1y,
    G2xIm,
    G2xRe,
    G2yIm,
    G2yRe,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Mutation {
    None,         // Generates combination of valid random pairs (P,Q) and/or inverse pairs (P,Q), (-P,Q) 
    Flip(Coord),  // Bit flip mutation
    Zero(Coord),  // Zeroes out a particular coordinate of P, Q points (see Coord enum)
    AllZeroG1,    // Zeroes out a particular P point
    AllZeroG2,    // Zeroes out a particular Q point
}

#[derive(Arbitrary, Debug, Clone)]
struct Input {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=2))]
    n_pairs_on_curve: usize,

    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=1))]
    n_inverse_blocks: usize,

    shuffle: bool,

    mutation: Mutation,
    bit_idx: u8,

    trunc: Option<u16>,
}

#[inline]
fn off_g1x(i: usize) -> usize {
    i * PAIR_SIZE + 0
}
#[inline]
fn off_g1y(i: usize) -> usize {
    i * PAIR_SIZE + 32
}
#[inline]
fn off_g2x_im(i: usize) -> usize {
    i * PAIR_SIZE + 64
}
#[inline]
fn off_g2x_re(i: usize) -> usize {
    i * PAIR_SIZE + 96
}
#[inline]
fn off_g2y_im(i: usize) -> usize {
    i * PAIR_SIZE + 128
}
#[inline]
fn off_g2y_re(i: usize) -> usize {
    i * PAIR_SIZE + 160
}

#[inline]
fn total_pairs(i: &Input) -> usize {
    let mut inv = i.n_inverse_blocks;
    if i.n_pairs_on_curve == 0 && inv == 0 {
        inv = 1;
    }
    i.n_pairs_on_curve + 2 * inv
}

#[inline]
fn fq_to_be32(x: Fq) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = x.into_bigint().to_bytes_be();
    let n = bytes.len();
    out[32 - n..].copy_from_slice(&bytes);
    out
}

#[inline]
fn fq2_split_be32(x: Fq2) -> ([u8; 32], [u8; 32]) {
    let re = fq_to_be32(x.c0);
    let im = fq_to_be32(x.c1);
    (im, re)
}

pub fn encode_g1(p: G1Affine) -> [u8; 64] {
    let mut out = [0u8; 64];
    if p.is_zero() {
        return out;
    }
    let x = fq_to_be32(p.x);
    let y = fq_to_be32(p.y);
    out[..32].copy_from_slice(&x);
    out[32..64].copy_from_slice(&y);
    out
}

pub fn encode_g2(p: G2Affine) -> [u8; 128] {
    let mut out = [0u8; 128];
    if p.is_zero() {
        return out;
    }
    let (x0, x1) = fq2_split_be32(p.x);
    let (y0, y1) = fq2_split_be32(p.y);
    out[0..32].copy_from_slice(&x0);
    out[32..64].copy_from_slice(&x1);
    out[64..96].copy_from_slice(&y0);
    out[96..128].copy_from_slice(&y1);
    out
}

fn apply_mut(buf: &mut [u8], n_pairs: usize, which: Mutation, bit_idx: u8) {
    let p = (bit_idx as usize) % n_pairs;
    let bi = (bit_idx as usize) & 31;
    let flip_at = |chunk: &mut [u8]| {
        chunk[bi] ^= 1;
    };

    match which {
        Mutation::None => {}
        Mutation::Flip(c) => match c {
            Coord::G1x => flip_at(&mut buf[off_g1x(p)..off_g1x(p) + CHUNK]),
            Coord::G1y => flip_at(&mut buf[off_g1y(p)..off_g1y(p) + CHUNK]),
            Coord::G2xIm => flip_at(&mut buf[off_g2x_im(p)..off_g2x_im(p) + CHUNK]),
            Coord::G2xRe => flip_at(&mut buf[off_g2x_re(p)..off_g2x_re(p) + CHUNK]),
            Coord::G2yIm => flip_at(&mut buf[off_g2y_im(p)..off_g2y_im(p) + CHUNK]),
            Coord::G2yRe => flip_at(&mut buf[off_g2y_re(p)..off_g2y_re(p) + CHUNK]),
        },
        Mutation::Zero(c) => match c {
            Coord::G1x => buf[off_g1x(p)..off_g1x(p) + CHUNK].fill(0),
            Coord::G1y => buf[off_g1y(p)..off_g1y(p) + CHUNK].fill(0),
            Coord::G2xIm => buf[off_g2x_im(p)..off_g2x_im(p) + CHUNK].fill(0),
            Coord::G2xRe => buf[off_g2x_re(p)..off_g2x_re(p) + CHUNK].fill(0),
            Coord::G2yIm => buf[off_g2y_im(p)..off_g2y_im(p) + CHUNK].fill(0),
            Coord::G2yRe => buf[off_g2y_re(p)..off_g2y_re(p) + CHUNK].fill(0),
        },
        Mutation::AllZeroG1 => {
            buf[off_g1x(p)..off_g1x(p) + G1_SIZE].fill(0);
        }
        Mutation::AllZeroG2 => {
            buf[off_g2x_im(p)..off_g2x_im(p) + G2_SIZE].fill(0);
        }
    }
}

pub fn gen_pairs(
    u: &mut arbitrary::Unstructured<'_>,
    n_pairs_on_curve: usize,
    n_inverse_blocks: usize,
    shuffle: bool,
) -> Vec<u8> {
    let g1 = G1Projective::generator();
    let g2 = G2Projective::generator();

    type BigInt = <ark_bn254::Fr as PrimeField>::BigInt;
    let bi = |x: u64| -> BigInt { x.into() };

    let mut inv = n_inverse_blocks;
    if n_pairs_on_curve == 0 && inv == 0 { inv = 1; }

    let total = n_pairs_on_curve + 2 * inv;
    let mut out = Vec::with_capacity(total * PAIR_SIZE);

    // random valid (P,Q)
    for _ in 0..n_pairs_on_curve {
        let s1: u16 = u.arbitrary().unwrap_or(1).max(1);
        let s2: u16 = u.arbitrary().unwrap_or(1).max(1);
        let p = (g1.mul_bigint(bi(s1 as u64))).into_affine();
        let q = (g2.mul_bigint(bi(s2 as u64))).into_affine();
        out.extend_from_slice(&encode_g1(p));
        out.extend_from_slice(&encode_g2(q));
    }

    // inverse blocks: (P,Q), (-P,Q)
    for _ in 0..inv {
        let s1: u16 = u.arbitrary().unwrap_or(2).max(1);
        let s2: u16 = u.arbitrary().unwrap_or(3).max(1);
        let p = (g1.mul_bigint(bi(s1 as u64))).into_affine();
        let q = (g2.mul_bigint(bi(s2 as u64))).into_affine();
        let p_neg = (-G1Projective::from(p)).into_affine();

        out.extend_from_slice(&encode_g1(p));
        out.extend_from_slice(&encode_g2(q));
        out.extend_from_slice(&encode_g1(p_neg));
        out.extend_from_slice(&encode_g2(q));
    }

    if shuffle {
        let mut i = total;
        while i > 1 {
            i -= 1;
            let j = (u.arbitrary::<u32>().unwrap_or(0) as usize) % i;
            
            let a = i * PAIR_SIZE;
            let b = j * PAIR_SIZE;

            let (left, right) = out.split_at_mut(a);
            let s1 = &mut left[b .. b + PAIR_SIZE];
            let s2 = &mut right[.. PAIR_SIZE];
            s1.swap_with_slice(s2);
        }
    }

    out
}

fn build_input(u: &mut Unstructured<'_>, i: &Input) -> Vec<u8> {
    let mut out = gen_pairs(u, i.n_pairs_on_curve, i.n_inverse_blocks, i.shuffle);

    let n_pairs = total_pairs(i);
    apply_mut(&mut out, n_pairs, i.mutation, i.bit_idx);

    if let Some(t) = i.trunc {
        let t = t as usize;
        if t < out.len() {
            out.truncate(t);
        }
    }
    out
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

    let r_reth = run_pair(in_bytes.as_slice(), 0, 0, u64::MAX);
    let reth_ok = r_reth.as_ref().is_ok_and(|x| !x.reverted);

    let r1 = pairing_forward(in_bytes.as_slice(), &mut out_forward);
    let fwd_ok = r1.is_ok();
    let r2 = pairing_proving(in_bytes.as_slice(), &mut out_proving);
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
    fuzz(data);
});
