pub mod consts;
pub mod curves;
pub mod fields;

pub use self::curves::{g1, g2, G1Affine, G1Projective, G2Affine, G2Projective};
pub use self::fields::{Fq, Fq12, Fq2, Fq6, Fr};

pub(crate) use self::curves::util;

use crate::ark_ec::pairing::Pairing;
use crate::ark_ec::AffineRepr;
use crate::ark_ff::{Field, PrimeField};
use consts::{G2_BY_TAU_POINT, PREPARED_G2_GENERATOR};

#[inline(always)]
pub fn verify_kzg_proof(
    commitment: G1Affine,
    proof: G1Affine,
    z: <Fr as PrimeField>::BigInt,
    y: <Fr as PrimeField>::BigInt,
) -> bool {
    // e(y - P, G₂) * e(proof, X - z) == 1
    let mut y_minus_p = G1Affine::generator().mul_bigint(&y);
    y_minus_p -= &commitment;

    let mut g2_el: G2Projective = G2_BY_TAU_POINT.into();
    let z_in_g2 = G2Affine::generator().mul_bigint(&z);
    g2_el -= z_in_g2;

    use crate::ark_ec::CurveGroup;
    let y_minus_p_prepared: G1Affine = y_minus_p.into_affine();
    let g2_el: <curves::Bls12_381 as Pairing>::G2Prepared = g2_el.into_affine().into();

    let gt_el = curves::Bls12_381::multi_pairing(
        [y_minus_p_prepared, proof],
        [PREPARED_G2_GENERATOR.clone(), g2_el],
    );
    gt_el.0 == <curves::Bls12_381 as Pairing>::TargetField::ONE
}
