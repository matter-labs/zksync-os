use core::u64;

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
use crate::ark_ff_delegation::MontFp;
use ark_ec::{
    bn,
    models::{short_weierstrass::SWCurveConfig, CurveConfig},
    scalar_mul::glv::GLVConfig,
    short_weierstrass::{Affine, Projective},
    AffineRepr,
};
#[cfg(not(any(all(target_arch = "riscv32", feature = "bigint_ops"), test)))]
use ark_ff::MontFp;
use ark_ff::{AdditiveGroup, BigInt, Field, PrimeField, Zero};
use ruint::aliases::U512;

use crate::{
    bn254::fields::{Fq, Fr},
    glv_decomposition::GLVConfigNoAllocator,
};

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Config;

pub type G1Affine = Affine<Config>;

impl CurveConfig for Config {
    type BaseField = Fq;
    type ScalarField = Fr;

    /// COFACTOR = 1
    const COFACTOR: &'static [u64] = &[0x1];

    /// COFACTOR_INV = COFACTOR^{-1} mod r = 1
    const COFACTOR_INV: Fr = Fr::ONE;
}

impl SWCurveConfig for Config {
    /// COEFF_A = 0
    const COEFF_A: Fq = Fq::ZERO;

    /// COEFF_B = 3
    const COEFF_B: Fq = MontFp!("3");

    /// AFFINE_GENERATOR_COEFFS = (G1_GENERATOR_X, G1_GENERATOR_Y)
    const GENERATOR: G1Affine = G1Affine::new_unchecked(G1_GENERATOR_X, G1_GENERATOR_Y);

    #[inline(always)]
    fn mul_by_a(_: Self::BaseField) -> Self::BaseField {
        Self::BaseField::zero()
    }

    #[inline]
    fn mul_projective(
        p: &bn::G1Projective<super::Config>,
        scalar: &[u64],
    ) -> bn::G1Projective<super::Config> {
        let s = Self::ScalarField::from_sign_and_limbs(true, scalar);
        GLVConfig::glv_mul_projective(*p, s)
    }

    #[inline]
    fn mul_affine(base: &Affine<Self>, scalar: &[u64]) -> bn::G1Projective<super::Config> {
        Self::mul_projective(&base.into_group(), scalar)
    }

    #[inline]
    fn is_in_correct_subgroup_assuming_on_curve(_p: &G1Affine) -> bool {
        // G1 = E(Fq) so if the point is on the curve, it is also in the subgroup.
        true
    }
}

impl GLVConfig for Config {
    const ENDO_COEFFS: &'static [Self::BaseField] = &[MontFp!(
        "21888242871839275220042445260109153167277707414472061641714758635765020556616"
    )];

    const LAMBDA: Self::ScalarField = ark_ff::MontFp!(
        "21888242871839275217838484774961031246154997185409878258781734729429964517155"
    );

    const SCALAR_DECOMP_COEFFS: [(bool, <Self::ScalarField as PrimeField>::BigInt); 4] = [
        (false, BigInt!("147946756881789319000765030803803410728")),
        (true, BigInt!("9931322734385697763")),
        (false, BigInt!("9931322734385697763")),
        (false, BigInt!("147946756881789319010696353538189108491")),
    ];

    fn endomorphism(p: &Projective<Self>) -> Projective<Self> {
        let mut res = (*p).clone();
        res.x *= Self::ENDO_COEFFS[0];
        res
    }
    fn endomorphism_affine(p: &Affine<Self>) -> Affine<Self> {
        let mut res = (*p).clone();
        res.x *= Self::ENDO_COEFFS[0];
        res
    }

    fn scalar_decomposition(
        k: Self::ScalarField,
    ) -> ((bool, Self::ScalarField), (bool, Self::ScalarField)) {
        Self::scalar_decomposition_no_allocator(k)
    }
}

impl GLVConfigNoAllocator for Config {
    const BETA_1: (bool, U512) = (
        false,
        U512::from_limbs([
            7440537858994729442,
            12177485554411886469,
            1601953548471081566,
            1485435879091901900,
            6023842690951505253,
            5534624963584316114,
            2,
            0,
        ]),
    );

    const BETA_2: (bool, U512) = (
        false,
        U512::from_limbs([
            10866705332225114937,
            3332646303595026058,
            10351474459561409124,
            7978627105577135858,
            15644699364383830999,
            2,
            0,
            0,
        ]),
    );
}

/// G1_GENERATOR_X = 1
pub const G1_GENERATOR_X: Fq = Fq::ONE;

/// G1_GENERATOR_Y = 2
pub const G1_GENERATOR_Y: Fq = MontFp!("2");

#[cfg(test)]
mod tests {
    use super::GLVConfigNoAllocator;
    use super::{Config, CurveConfig, GLVConfig, PrimeField};
    use proptest::{prop_assert_eq, proptest};
    type ScalarField = <Config as CurveConfig>::ScalarField;

    #[test]
    fn compare_scalar_decomposition() {
        proptest!(|(bytes: [u8; 32])| {
            let k = ScalarField::from_be_bytes_mod_order(&bytes);

            let (k1, k2) = Config::scalar_decomposition(k.clone());
            let (k1_ref, k2_ref) = Config::scalar_decomposition_ref(k);

            prop_assert_eq!(k1, k1_ref);
            prop_assert_eq!(k2, k2_ref);
        })
    }

    #[test]
    fn test_betas() {
        use ark_std::ops::Neg;
        use num_bigint::{BigInt, BigUint, Sign};
        use num_integer::Integer;
        use ruint::aliases::U512;

        let coeff_bigints: [BigInt; 4] = Config::SCALAR_DECOMP_COEFFS.map(|x| {
            BigInt::from_biguint(x.0.then_some(Sign::Plus).unwrap_or(Sign::Minus), x.1.into())
        });

        let [_, n12, _, n22] = coeff_bigints;

        let n = 512u64;
        let r = BigInt::from(<<Config as CurveConfig>::ScalarField>::MODULUS);

        let beta_1_ref = (n22 << n).div_rem(&r).0;

        let sign = Config::BETA_1
            .0
            .then_some(Sign::Plus)
            .unwrap_or(Sign::Minus);
        let data = BigUint::from_bytes_be(&Config::BETA_1.1.to_be_bytes::<{ U512::BYTES }>());
        let beta_1 = BigInt::from_biguint(sign, data);
        assert_eq!(beta_1, beta_1_ref);

        let beta_2_ref = ((n12 << n).neg()).div_rem(&r).0;

        let sign = Config::BETA_2
            .0
            .then_some(Sign::Plus)
            .unwrap_or(Sign::Minus);
        let data = BigUint::from_bytes_be(&Config::BETA_2.1.to_be_bytes::<{ U512::BYTES }>());
        let beta_2 = BigInt::from_biguint(sign, data);
        assert_eq!(beta_2, beta_2_ref);
    }
}
