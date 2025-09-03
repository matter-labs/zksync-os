use core::{array, fmt::Debug};

use ark_ec::{scalar_mul::glv::GLVConfig, CurveConfig};
use ark_ff::PrimeField;
use ruint::aliases::{U1024, U512};

// default ark implementation for scalar decomposition uses global allocator, so we need to write our own
pub(crate) trait GLVConfigNoAllocator: GLVConfig
where
    <<Self as CurveConfig>::ScalarField as PrimeField>::BigInt: AsRef<[u64]>,
{
    /// BETA_1 = n22 * 2^512 / modulus
    const BETA_1: (bool, U512);
    /// BETA_2 = -n12 * 2^512 / modulus
    const BETA_2: (bool, U512);

    // TODO(yoaveshel):
    //  - change to delegated U256
    //  - using U512 everywhere is probably overkill
    #[inline(always)]
    fn scalar_decomposition_no_allocator(
        k: Self::ScalarField,
    ) -> ((bool, Self::ScalarField), (bool, Self::ScalarField)) {
        let s = I512::from_limbs_slice_and_sign(true, k.into_bigint().as_ref());

        let [n11, n12, n21, n22] = array::from_fn(|i| {
            let (sign, bigint) = Self::SCALAR_DECOMP_COEFFS[i];
            I512::from_limbs_slice_and_sign(sign, bigint.as_ref())
        });

        let beta_1 = s.mul_and_shift(&Self::BETA_1.into());
        let beta_2 = s.mul_and_shift(&Self::BETA_2.into());

        let b11 = beta_1.mul(&n11);
        let b12 = beta_2.mul(&n21);
        let b1 = b11.add(&b12);

        let b21 = beta_1.mul(&n12);
        let b22 = beta_2.mul(&n22);
        let b2 = b21.add(&b22);

        let k1 = s.sub(&b1);
        let k2 = b2.neg();

        (
            (
                k1.sign,
                Self::ScalarField::from_le_bytes_mod_order(
                    &k1.data.to_le_bytes::<{ U512::BYTES }>(),
                ),
            ),
            (
                k2.sign,
                Self::ScalarField::from_le_bytes_mod_order(
                    &k2.data.to_le_bytes::<{ U512::BYTES }>(),
                ),
            ),
        )
    }

    // default implementation from ark for comparison
    #[cfg(test)]
    fn scalar_decomposition_ref(
        k: Self::ScalarField,
    ) -> ((bool, Self::ScalarField), (bool, Self::ScalarField)) {
        use ark_std::ops::{AddAssign, Neg};
        use num_bigint::{BigInt, BigUint, Sign};
        use num_integer::Integer;
        use num_traits::{One, Signed};

        let scalar: BigInt = k.into_bigint().into().into();

        let coeff_bigints: [BigInt; 4] = Self::SCALAR_DECOMP_COEFFS.map(|x| {
            BigInt::from_biguint(x.0.then_some(Sign::Plus).unwrap_or(Sign::Minus), x.1.into())
        });

        let [n11, n12, n21, n22] = coeff_bigints;

        let r = BigInt::from(<<Self as CurveConfig>::ScalarField>::MODULUS.into());

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
            (
                k1.sign() == Sign::Plus,
                <<Self as CurveConfig>::ScalarField>::from(k1_abs),
            ),
            (
                k2.sign() == Sign::Plus,
                <<Self as CurveConfig>::ScalarField>::from(k2_abs),
            ),
        )
    }
}

pub struct I512 {
    pub sign: bool,
    pub data: U512,
}

impl Debug for I512 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let sign = if self.sign { "" } else { "-" };
        f.write_fmt(format_args!("{}{}", sign, self.data))
    }
}

impl I512 {
    #[inline(always)]
    pub fn from_limbs_slice_and_sign(sign: bool, slice: &[u64]) -> Self {
        let data = U512::from_limbs_slice(slice);
        Self { sign, data }
    }

    #[inline(always)]
    pub fn mul_and_shift(&self, rhs: &Self) -> Self {
        let wide_prod: U1024 = self.data.widening_mul(rhs.data);
        let limbs = wide_prod.as_limbs();

        let mut high = U512::from_limbs(limbs[8..].try_into().unwrap());
        let sign = !(self.sign ^ rhs.sign);

        // Round up for positive results when lower half >= 2^511
        if sign && limbs[7] >> 63 == 1 {
            high = high.wrapping_add(U512::ONE);
        }

        Self { sign, data: high }
    }

    #[inline(always)]
    pub fn mul(&self, rhs: &Self) -> Self {
        let data = self.data.checked_mul(rhs.data).unwrap();
        Self {
            sign: !(self.sign ^ rhs.sign),
            data,
        }
    }

    #[inline(always)]
    pub fn add(&self, rhs: &Self) -> Self {
        match (self.sign, rhs.sign) {
            (true, false) | (false, true) => match self.data.cmp(&rhs.data) {
                core::cmp::Ordering::Less => Self {
                    sign: rhs.sign,
                    data: rhs.data.checked_sub(self.data).unwrap(),
                },
                core::cmp::Ordering::Greater => Self {
                    sign: self.sign,
                    data: self.data.checked_sub(rhs.data).unwrap(),
                },
                core::cmp::Ordering::Equal => Self {
                    sign: false,
                    data: U512::ZERO,
                },
            },
            (true, true) | (false, false) => Self {
                sign: self.sign,
                data: self.data.checked_add(rhs.data).unwrap(),
            },
        }
    }

    #[inline(always)]
    pub fn sub(&self, rhs: &Self) -> Self {
        self.add(&rhs.neg())
    }

    #[inline(always)]
    pub fn neg(&self) -> Self {
        Self {
            sign: !self.sign,
            data: self.data,
        }
    }
}

impl From<(bool, U512)> for I512 {
    fn from(value: (bool, U512)) -> Self {
        Self {
            sign: value.0,
            data: value.1,
        }
    }
}
