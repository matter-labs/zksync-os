// EIP-2537 serialization helpers
// These are defined in the crypto crate to avoid ICE when compiling for RISC-V
// The issue is that functions in external crates that use Fq/G1/G2 types trigger
// compiler bugs during predicate checking for generic constants.

use super::{Fq, Fq2, G1Affine, G2Affine};
use crate::ark_ec::AffineRepr;
use crate::ark_ff::PrimeField;

const FIELD_ELEMENT_LEN: usize = 64;
const G1_LEN: usize = 128;
const G2_LEN: usize = 256;

#[inline(never)]
pub fn parse_fq_bytes(input: &[u8; FIELD_ELEMENT_LEN]) -> Option<Fq> {
    if input[..16].iter().all(|el| *el == 0) == false {
        return None;
    }
    let mut repr = <Fq as PrimeField>::BigInt::zero();
    let repr_slice = repr.as_mut();
    for (dst, src) in repr_slice
        .iter_mut()
        .zip(input[16..].as_rchunks::<8>().1.iter().rev())
    {
        *dst = u64::from_be_bytes(*src);
    }
    Fq::from_bigint(repr)
}

#[inline(never)]
pub fn parse_fq2_bytes(input: &[u8; FIELD_ELEMENT_LEN * 2]) -> Option<Fq2> {
    let c0 = parse_fq_bytes(input[0..64].try_into().ok()?)?;
    let c1 = parse_fq_bytes(input[64..128].try_into().ok()?)?;
    Some(Fq2 { c0, c1 })
}

#[inline(never)]
pub fn parse_g1_bytes(input: &[u8; G1_LEN]) -> Option<(G1Affine, bool)> {
    if input.iter().all(|el| *el == 0) {
        return Some((G1Affine::identity(), false));
    }
    let x = parse_fq_bytes(input[0..64].try_into().ok()?)?;
    let y = parse_fq_bytes(input[64..128].try_into().ok()?)?;
    let point = G1Affine::new_unchecked(x, y);

    if !point.is_on_curve() {
        return None;
    }

    Some((point, true))
}

#[inline(never)]
pub fn parse_g2_bytes(input: &[u8; G2_LEN]) -> Option<(G2Affine, bool)> {
    if input.iter().all(|el| *el == 0) {
        return Some((G2Affine::identity(), false));
    }
    let x = parse_fq2_bytes(input[0..128].try_into().ok()?)?;
    let y = parse_fq2_bytes(input[128..256].try_into().ok()?)?;
    let point = G2Affine::new_unchecked(x, y);

    if !point.is_on_curve() {
        return None;
    }

    Some((point, true))
}

#[inline(never)]
pub fn serialize_fq_bytes(el: Fq, output: &mut [u8; FIELD_ELEMENT_LEN]) {
    output[..16].fill(0);
    let bigint = el.into_bigint();
    let words = bigint.as_ref();
    for (i, word) in words.iter().take(6).enumerate() {
        let bytes = word.to_be_bytes();
        let start = 16 + (5 - i) * 8;
        output[start..start + 8].copy_from_slice(&bytes);
    }
}

#[inline(never)]
pub fn serialize_fq2_bytes(el: Fq2, output: &mut [u8; FIELD_ELEMENT_LEN * 2]) {
    let (left, right) = output.split_at_mut(64);
    serialize_fq_bytes(el.c0, left.try_into().unwrap());
    serialize_fq_bytes(el.c1, right.try_into().unwrap());
}

#[inline(never)]
pub fn serialize_g1_bytes(el: G1Affine, output: &mut [u8; G1_LEN]) {
    if let Some((x, y)) = el.xy() {
        let (left, right) = output.split_at_mut(64);
        serialize_fq_bytes(x, left.try_into().unwrap());
        serialize_fq_bytes(y, right.try_into().unwrap());
    } else {
        output.fill(0);
    }
}

#[inline(never)]
pub fn serialize_g2_bytes(el: G2Affine, output: &mut [u8; G2_LEN]) {
    if let Some((x, y)) = el.xy() {
        let (left, right) = output.split_at_mut(128);
        serialize_fq2_bytes(x, left.try_into().unwrap());
        serialize_fq2_bytes(y, right.try_into().unwrap());
    } else {
        output.fill(0);
    }
}
