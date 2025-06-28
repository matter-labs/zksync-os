#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
mod scalar_delegation;

#[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
mod scalar64;

use core::ops::{Mul, Neg};

cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))] {
        pub(super) use scalar_delegation::Scalar;
    } else {
        pub(super) use scalar64::Scalar;
    }

}

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub(super) use scalar_delegation::init;

use super::{wnaf::ToWnaf, Secp256r1Err};

// Curve order
const MODULUS: [u64; 4] = [
    17562291160714782033,
    13611842547513532036,
    18446744073709551615,
    18446744069414584320,
];

/// MU = floor(2^512 / n)
const MU: [u64; 5] = [
    0x012f_fd85_eedf_9bfe,
    0x4319_0552_df1a_6c21,
    0xffff_fffe_ffff_ffff,
    0x0000_0000_ffff_ffff,
    0x0000_0000_0000_0001,
];

#[allow(dead_code)]
const REDUCTION_CONST: [u64; 4] = [
    14758798090332847183,
    5244798044304888548,
    5836234025928804086,
    6976188194875648028,
];
#[allow(dead_code)]
const R2: [u64; 4] = [
    9449762124159643298,
    5087230966250696614,
    2901921493521525849,
    7413256579398063648,
];

impl Scalar {
    // https://github.com/RustCrypto/elliptic-curves/blob/master/p256/src/arithmetic/scalar.rs#L132
    pub(super) fn invert_assign(&mut self) {
        // We need to find b such that b * a ≡ 1 mod p. As we are in a prime
        // field, we can apply Fermat's Little Theorem:
        //
        //    a^p         ≡ a mod p
        //    a^(p-1)     ≡ 1 mod p
        //    a^(p-2) * a ≡ 1 mod p
        //
        // Thus inversion can be implemented with a single exponentiation.
        //
        // This is `n - 2`, so the top right two digits are `4f` instead of `51`.
        *self = self.pow_vartime(&[
            0xf3b9_cac2_fc63_254f,
            0xbce6_faad_a717_9e84,
            0xffff_ffff_ffff_ffff,
            0xffff_ffff_0000_0000,
        ])
    }

    #[inline(always)]
    // https://github.com/RustCrypto/elliptic-curves/blob/master/p256/src/arithmetic/scalar.rs#L153
    pub fn pow_vartime(&self, exp: &[u64]) -> Self {
        let mut res = Self::ONE;

        let mut i = exp.len();
        while i > 0 {
            i -= 1;

            let mut j = 64;
            while j > 0 {
                j -= 1;
                res.square_assign();

                if ((exp[i] >> j) & 1) == 1 {
                    res.mul_assign(self);
                }
            }
        }

        res
    }
}

impl Mul<&Self> for Scalar {
    type Output = Self;

    fn mul(mut self, rhs: &Self) -> Self::Output {
        self.mul_assign(rhs);
        self
    }
}

impl Neg for Scalar {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        self.neg_assign();
        self
    }
}

impl PartialEq for Scalar {
    fn eq(&self, other: &Self) -> bool {
        self.eq_inner(other)
    }
}

impl ToWnaf for Scalar {
    fn bits(&self, offset: usize, count: usize) -> u32 {
        // check requested bits must be from the same limb
        debug_assert!((offset + count - 1) >> 6 == offset >> 6);
        let limbs = self.to_words();
        ((limbs[offset >> 6] >> (offset & 0x3F)) & ((1 << count) - 1)) as u32
    }

    fn bits_var(&self, offset: usize, count: usize) -> u32 {
        debug_assert!(count <= 32);
        debug_assert!(offset + count <= 256);
        // if all the requested bits are in the same limb
        if (offset + count - 1) >> 6 == offset >> 6 {
            self.bits(offset, count)
        } else {
            debug_assert!((offset >> 6) + 1 < 4);
            let limbs = self.to_words();
            (((limbs[offset >> 6] >> (offset & 0x3F))
                | (limbs[(offset >> 6) + 1] << (64 - (offset & 0x3F))))
                & ((1 << count) - 1)) as u32
        }
    }
}

pub(super) struct Signature {
    pub(super) r: Scalar,
    pub(super) s: Scalar,
}

impl Signature {
    pub(super) fn from_scalars(r: &[u8; 32], s: &[u8; 32]) -> Result<Self, Secp256r1Err> {
        let r = Scalar::from_be_bytes(r)?;
        let s = Scalar::from_be_bytes(s)?;

        if r.is_zero() || s.is_zero() {
            Err(Secp256r1Err::InvalidSignature)
        } else {
            Ok(Self { r, s })
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::{prop_assert, prop_assert_eq, proptest};

    use super::*;

    #[cfg(feature = "bigint_ops")]
    fn init() {
        crate::secp256r1::init();
        crate::bigint_delegation::init();
    }

    #[test]
    fn test_mul() {
        #[cfg(feature = "bigint_ops")]
        init();

        proptest!(|(x: Scalar, y: Scalar, z: Scalar)| {
            prop_assert_eq!(x * &Scalar::ONE, x);
            prop_assert_eq  !(x * &Scalar::ZERO, Scalar::ZERO);
            prop_assert_eq!(x * &y, y * &x);
            prop_assert_eq!((x * &y) * &z, x * &(y * &z))
        })
    }

    #[test]
    fn test_invert() {
        #[cfg(feature = "bigint_ops")]
        init();

        proptest!(|(s: Scalar)| {
            let mut a = s;
            a.invert_assign();
            a.invert_assign();
            prop_assert_eq!(a, s);

            a.invert_assign();
            a.mul_assign(&s);
            if s.is_zero() {
                prop_assert!(a.is_zero())
            } else {
                prop_assert_eq!(a, Scalar::ONE);
            }
        })
    }
}
