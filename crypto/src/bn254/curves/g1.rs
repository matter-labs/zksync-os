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

use crate::bn254::fields::{Fq, Fr};

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

    // TODO(yoaveshel):
    //  - change to delegated U256
    //  - using U512 everywhere is overkill (e.g. mul_and_shift can return U256)
    fn scalar_decomposition(
        k: Self::ScalarField,
    ) -> ((bool, Self::ScalarField), (bool, Self::ScalarField)) {
        use ruint::aliases::{U1024, U512};

        fn mul_and_shift(lhs: &U512, rhs: U512) -> U512 {
            let x: U1024 = lhs.widening_mul(rhs);
            U512::from_limbs(x.as_limbs().split_at(8).1.try_into().unwrap())
        }

        fn sub(lhs: U512, rhs: U512) -> (bool, U512) {
            if lhs > rhs {
                (true, lhs.wrapping_sub(rhs))
            } else {
                (false, rhs.wrapping_sub(lhs))
            }
        }

        // BETA_1 = n22 * 2^512 / modulus
        const BETA_1: U512 = U512::from_limbs([
            7440537858994729442,
            12177485554411886469,
            1601953548471081566,
            1485435879091901900,
            6023842690951505253,
            5534624963584316114,
            2,
            0,
        ]);
        // BETA_2 = n12 * 2^512 / modulus
        const BETA_2: U512 = U512::from_limbs([
            10866705332225114937,
            3332646303595026058,
            10351474459561409124,
            7978627105577135858,
            15644699364383830999,
            2,
            0,
            0,
        ]);

        let s = U512::from_limbs_slice(&k.into_bigint().0);

        let n11 = U512::from_limbs_slice(&Self::SCALAR_DECOMP_COEFFS[0].1 .0);
        let n12 = U512::from_limbs_slice(&Self::SCALAR_DECOMP_COEFFS[1].1 .0);
        let n21 = U512::from_limbs_slice(&Self::SCALAR_DECOMP_COEFFS[2].1 .0);
        let n22 = U512::from_limbs_slice(&Self::SCALAR_DECOMP_COEFFS[3].1 .0);

        let beta_1 = mul_and_shift(&BETA_1, s);
        let beta_2 = mul_and_shift(&BETA_2, s);

        let b11 = beta_1.wrapping_mul(n11);
        let b12 = beta_2.wrapping_mul(n21);
        let b1 = b11.wrapping_add(b12);

        let b21 = beta_1.wrapping_mul(n12);
        let b22 = beta_2.wrapping_mul(n22);
        let (b2_sign, b2) = sub(b22, b21);

        let (k1_sign, k1) = sub(s, b1);

        let (k2_sign, k2) = (!b2_sign, b2);

        let k1 = Self::ScalarField::from_le_bytes_mod_order(&k1.to_le_bytes::<{ U512::BYTES }>());
        let k2 = Self::ScalarField::from_le_bytes_mod_order(&k2.to_le_bytes::<{ U512::BYTES }>());

        ((k1_sign, k1), (k2_sign, k2))
    }
}

/// G1_GENERATOR_X = 1
pub const G1_GENERATOR_X: Fq = Fq::ONE;

/// G1_GENERATOR_Y = 2
pub const G1_GENERATOR_Y: Fq = MontFp!("2");

#[cfg(test)]
mod tests {
    use super::{Config, CurveConfig, GLVConfig, PrimeField};
    use proptest::{prop_assert_eq, proptest};

    type ScalarField = <Config as CurveConfig>::ScalarField;

    #[test]
    fn compare_scalar_decomposition() {
        proptest!(|(bytes: [u8; 32])| {
            let k = ScalarField::from_be_bytes_mod_order(&bytes);

            let (k1, k2) = <Config as GLVConfig>::scalar_decomposition(k.clone());
            let (k1_ref, k2_ref) = scalar_decomposition_ref(k);

            prop_assert_eq!(k1, k1_ref);
            prop_assert_eq!(k2, k2_ref);
        })
    }

    // default implementation from ark-ec
    fn scalar_decomposition_ref(k: ScalarField) -> ((bool, ScalarField), (bool, ScalarField)) {
        use ark_std::ops::{AddAssign, Neg};
        use num_bigint::{BigInt, BigUint, Sign};
        use num_integer::Integer;
        use num_traits::{One, Signed};

        let scalar: BigInt = k.into_bigint().into();

        let coeff_bigints: [BigInt; 4] = Config::SCALAR_DECOMP_COEFFS.map(|x| {
            BigInt::from_biguint(x.0.then_some(Sign::Plus).unwrap_or(Sign::Minus), x.1.into())
        });

        let [n11, n12, n21, n22] = coeff_bigints;

        let r = BigInt::from(ScalarField::MODULUS);

        // beta = vector([k,0]) * self.curve.N_inv
        // The inverse of N is 1/r * Matrix([[n22, -n12], [-n21, n11]]).
        // so β = (k*n22, -k*n12)/r

        let beta_1 = {
            let (mut div, rem) = (&scalar * &n22).div_rem(&r);
            if (&rem + &rem) > r {
                div.add_assign(BigInt::one());
            }
            div
        };
        let beta_2 = {
            let (mut div, rem) = (&scalar * &n12.clone().neg()).div_rem(&r);
            if (&rem + &rem) > r {
                div.add_assign(BigInt::one());
            }
            div
        };

        // b = vector([int(beta[0]), int(beta[1])]) * self.curve.N
        // b = (β1N11 + β2N21, β1N12 + β2N22) with the signs!
        //   = (b11   + b12  , b21   + b22)   with the signs!

        // b1
        let b11 = &beta_1 * &n11;
        let b12 = &beta_2 * &n21;
        let b1 = b11 + b12;

        // b2
        let b21 = &beta_1 * &n12;
        let b22 = &beta_2 * &n22;
        let b2 = b21 + b22;

        let k1 = &scalar - b1;
        let k1_abs = BigUint::try_from(k1.abs()).unwrap();

        // k2
        let k2 = -b2;
        let k2_abs = BigUint::try_from(k2.abs()).unwrap();

        (
            (k1.sign() == Sign::Plus, ScalarField::from(k1_abs)),
            (k2.sign() == Sign::Plus, ScalarField::from(k2_abs)),
        )
    }
}
