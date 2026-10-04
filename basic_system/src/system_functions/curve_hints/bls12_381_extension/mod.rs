//! Checked BLS12-381 extension-field operations using the existing base-field hints.

use super::{bls12_381_fq_inverse, bls12_381_fq_sqrt};
use core::mem::MaybeUninit;
use crypto::ark_ff::{AdditiveGroup, Field, PrimeField, Zero};
use crypto::bls12_381::{Fq, Fq2};
use zk_ee::oracle::IOOracle;

/// Inverse in Fq[i], where i² = -1. The norm vanishes only for the zero element,
/// because -1 is not a square in the BLS12-381 base field.
pub fn bls12_381_fq2_inverse<O: IOOracle>(oracle: &mut O, value: &Fq2) -> Option<Fq2> {
    let norm = value.c0.square() + value.c1.square();
    let inverse = bls12_381_fq_inverse(oracle, &norm)?;
    Some(Fq2::new(value.c0 * inverse, -(value.c1 * inverse)))
}

/// Extension-field division with a checked hint for the inverse of the denominator's norm.
pub struct Bls12381Fq2Divider<'a, O: IOOracle> {
    oracle: &'a mut O,
}

impl<'a, O: IOOracle> Bls12381Fq2Divider<'a, O> {
    pub fn new(oracle: &'a mut O) -> Self {
        Self { oracle }
    }
}

impl<O: IOOracle> crypto::affine_glv::Divider<Fq2> for Bls12381Fq2Divider<'_, O> {
    fn divide<'q>(
        &mut self,
        fraction: &mut [Fq2; 2],
        quotient: &'q mut MaybeUninit<Fq2>,
    ) -> &'q mut Fq2 {
        // Divider's caller guarantees a nonzero denominator; exceptional affine cases
        // are handled before asking for a division.
        let inverse = bls12_381_fq2_inverse(self.oracle, &fraction[1])
            .expect("the affine division denominator is nonzero");
        quotient.write(fraction[0] * inverse)
    }
}

/// A square root in Fq2, or a checked determination that none exists.
/// Every queried root/inverse is checked by the base-field helper. The root's sign
/// is unspecified; callers such as SWU choose the prescribed sign afterwards.
pub fn bls12_381_fq2_sqrt<O: IOOracle>(oracle: &mut O, value: &Fq2) -> Option<Fq2> {
    if value.c1.is_zero() {
        // Every base-field element has a square root in Fq2. If a is a nonsquare
        // in Fq, -a is a square, and (sqrt(-a) * i)² = a.
        return match bls12_381_fq_sqrt(oracle, &value.c0) {
            Some(root) => Some(Fq2::new(root, Fq::ZERO)),
            None => bls12_381_fq_sqrt(oracle, &-value.c0).map(|root| Fq2::new(Fq::ZERO, root)),
        };
    }
    // An element of Fq2 is a square iff its norm is a square in Fq.
    let norm = value.c0.square() + value.c1.square();
    let norm_root = bls12_381_fq_sqrt(oracle, &norm)?;
    let half = -Fq::from_bigint(Fq::MODULUS_MINUS_ONE_DIV_TWO)
        .expect("half the modulus minus one is canonical");
    // The product of these two candidates is -c1²/4, a nonsquare. Exactly
    // one candidate has a square root, and neither is zero when c1 != 0.
    let first = (value.c0 + norm_root) * half;
    let root = match bls12_381_fq_sqrt(oracle, &first) {
        Some(root) => root,
        None => bls12_381_fq_sqrt(oracle, &((value.c0 - norm_root) * half))?,
    };
    let denominator = root.double();
    let inverse = bls12_381_fq_inverse(oracle, &denominator)
        .expect("a non-real square has a nonzero real root component");
    let result = Fq2::new(root, value.c1 * inverse);
    assert_eq!(
        result.square(),
        *value,
        "the extension-field square root is wrong"
    );
    Some(result)
}

#[cfg(test)]
mod tests;
