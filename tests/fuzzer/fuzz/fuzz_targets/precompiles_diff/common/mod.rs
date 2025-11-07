use arbitrary::Arbitrary;
use ark_bn254::{G1Affine, G1Projective, Fq, Fr};
use ark_ec::{AffineRepr,CurveGroup,Group};
use ark_ff::{PrimeField, BigInteger};

#[derive(Arbitrary, Debug, Clone, Copy)]
pub enum PointKind {
    Valid,
    MutatedValid,
    Infinity,
    RawBytes,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
pub enum CoordMut {
    None,
    FlipEndianness,
    PlusOneModP,
    MinusOneModP,
    OneBitFlip,
    ZeroCoord,
}

fn fq_to_be_bytes(x: Fq) -> [u8; 32] {
    let limbs = x.into_bigint().0;
    let mut out = [0u8; 32];
    for (i, limb) in limbs.iter().rev().enumerate() {
        out[i*8..(i+1)*8].copy_from_slice(&limb.to_be_bytes());
    }
    out
}

const P_BE: [u8; 32] = [
    0x30, 0x64, 0x4E, 0x72, 0xE1, 0x31, 0xA0, 0x29,
    0xB8, 0x50, 0x45, 0xB6, 0x81, 0x81, 0x58, 0x5D,
    0x28, 0x33, 0xE8, 0x48, 0x79, 0xB9, 0x70, 0x91,
    0x43, 0xE1, 0xF5, 0x93, 0xF0, 0x00, 0x00, 0x01,
];

#[inline]
fn be_lt(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for i in 0..32 {
        if a[i] < b[i] { return true; }
        if a[i] > b[i] { return false; }
    }
    false
}

fn be_bytes_to_fq(be: [u8; 32]) -> Option<Fq> {
    if !be_lt(&be, &P_BE) {
        return None;
    }
    Some(Fq::from_be_bytes_mod_order(&be))
}

fn plus_one_mod_p(x: Fq) -> Fq { x + Fq::from(1u32) }
fn minus_one_mod_p(x: Fq) -> Fq { x - Fq::from(1u32) }

fn make_valid_affine(s: Fr) -> G1Affine {
    let p: G1Projective = G1Projective::generator().mul_bigint(s.into_bigint());
    p.into_affine()
}

fn scalar_from_bytes_mod_r(bytes: [u8; 32]) -> Fr {
    Fr::from_be_bytes_mod_order(&bytes)
}

fn encode_point_affine(p: G1Affine) -> ([u8; 32], [u8; 32]) {
    if p.is_zero() {
        return ([0u8; 32], [0u8; 32]);
    }
    (fq_to_be_bytes(p.x), fq_to_be_bytes(p.y))
}

// Mutate coordinates independently of a valid curve point:
// 1. Swap endianness
// 2. Add/Subtract 1 mod p
// 3. Flip a random bit
// 4. Zero out
fn mutate_coord(bytes_be: [u8; 32], m: CoordMut) -> [u8; 32] {
    match m {
        CoordMut::None => bytes_be,
        CoordMut::FlipEndianness => {
            let mut v = bytes_be; v.reverse(); v
        }
        CoordMut::PlusOneModP => {
            if let Some(f) = be_bytes_to_fq(bytes_be) { fq_to_be_bytes(plus_one_mod_p(f)) } else { bytes_be }
        }
        CoordMut::MinusOneModP => {
            if let Some(f) = be_bytes_to_fq(bytes_be) { fq_to_be_bytes(minus_one_mod_p(f)) } else { bytes_be }
        }
        CoordMut::OneBitFlip => {
            let mut v = bytes_be;
            let idx = (v[0] as usize) & 31;
            v[idx] ^= 1;
            v
        }
        CoordMut::ZeroCoord => [0u8; 32],
    }
}

// Construct a curve point; it can be:
// 1. Infinity
// 2. Invalid — random bytes
// 3. Invalid — small mutation of a valid point
// 4. Valid
pub fn build_point_bytes(kind: PointKind, s_bytes: [u8; 32], mx: CoordMut, my: CoordMut, spice_mask: u8, raw64: [u8; 64]) -> [u8; 64] {
    match kind {
        PointKind::Infinity => [0u8; 64],

        PointKind::RawBytes => raw64,

        PointKind::Valid => {
            let s = scalar_from_bytes_mod_r(s_bytes);
            let (x, y) = encode_point_affine(make_valid_affine(s));
            let mut out = [0u8; 64];
            out[..32].copy_from_slice(&x);
            out[32..].copy_from_slice(&y);
            out
        }

        PointKind::MutatedValid => {
            let s = scalar_from_bytes_mod_r(s_bytes);
            let mut p = make_valid_affine(s);

            let (mut x, mut y) = encode_point_affine(p);
            if spice_mask & 0x01 != 0 { x = mutate_coord(x, mx); }
            if spice_mask & 0x02 != 0 { y = mutate_coord(y, my); }

            let mut out = [0u8; 64];
            out[..32].copy_from_slice(&x);
            out[32..].copy_from_slice(&y);
            out
        }
    }
}

#[inline]
pub fn be_inc_inplace(x: &mut [u8; 32]) {
    for i in (0..32).rev() {
        let (v, c) = x[i].overflowing_add(1);
        x[i] = v;
        if !c { break; }
    }
}

#[inline]
pub fn be_dec_inplace(x: &mut [u8; 32]) {
    for i in (0..32).rev() {
        let (v, b) = x[i].overflowing_sub(1);
        x[i] = v;
        if !b { break; }
    }
}