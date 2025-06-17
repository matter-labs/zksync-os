#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
mod scalar_delegation;

#[cfg(target_pointer_width = "64")]
mod scalar64;

use core::ops::{Mul, Neg};

#[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
pub(crate) use scalar_delegation::Scalar;

#[cfg(target_pointer_width = "64")]
pub(crate) use scalar64::Scalar;

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

const REDUCTION_CONST: [u64; 4] = [
    14758798090332847183,
    5244798044304888548,
    5836234025928804086,
    6976188194875648028,
];
const R2: [u64; 4] = [
    9449762124159643298,
    5087230966250696614,
    2901921493521525849,
    7413256579398063648,
];

impl Scalar {
    // https://www.briansmith.org/ecc-inversion-addition-chains-01#p256_scalar_inversion
    pub(super) fn invert_assign(&mut self) {
        let t_1 = *self;

        let mut t_10 = t_1;
        t_10.square_assign();

        self.mul_assign(&t_10);
        let t_11 = *self;

        self.mul_assign(&t_10);
        let t_101 = *self;

        let mut t_111 = t_101;
        t_111.mul_assign(&t_10);

        self.square_assign();
        let t_1010 = *self;

        let mut t_1111 = t_1010;
        t_1111.mul_assign(&t_101);

        self.square_assign();
        self.mul_assign(&t_1);
        let t_10101 = *self;

        self.square_assign();
        let t_101010 = *self;

        let mut t_101111 = t_101010;
        t_101111.mul_assign(&t_101);

        self.mul_assign(&t_10101);

        self.pow2k_assign(2);
        self.mul_assign(&t_11);
        let x8 = *self;

        self.pow2k_assign(8);
        self.mul_assign(&x8);
        let x16 = *self;

        self.pow2k_assign(16);
        self.mul_assign(&x16);
        let x32 = *self;

        self.pow2k_assign(64);
        self.mul_assign(&x32);
        self.pow2k_assign(32);
        self.mul_assign(&x32);
        self.pow2k_assign(6);
        self.mul_assign(&t_101111);
        self.pow2k_assign(5);
        self.mul_assign(&t_111);
        self.pow2k_assign(4);
        self.mul_assign(&t_11);
        self.pow2k_assign(5);
        self.mul_assign(&t_1111);
        self.pow2k_assign(5);
        self.mul_assign(&t_10101);
        self.pow2k_assign(4);
        self.mul_assign(&t_101);
        self.pow2k_assign(3);
        self.mul_assign(&t_101);
        self.pow2k_assign(3);
        self.mul_assign(&t_101);
        self.pow2k_assign(5);
        self.mul_assign(&t_111);
        self.pow2k_assign(9);
        self.mul_assign(&t_101111);
        self.pow2k_assign(8);
        self.mul_assign(&t_1111);
        self.pow2k_assign(2);
        self.mul_assign(&t_1);
        self.pow2k_assign(5);
        self.mul_assign(&t_1);
        self.pow2k_assign(6);
        self.mul_assign(&t_1111);
        self.pow2k_assign(5);
        self.mul_assign(&t_111);
        self.pow2k_assign(4);
        self.mul_assign(&t_111);
        self.pow2k_assign(5);
        self.mul_assign(&t_111);
        self.pow2k_assign(5);
        self.mul_assign(&t_101);
        self.pow2k_assign(3);
        self.mul_assign(&t_11);
        self.pow2k_assign(10);
        self.mul_assign(&t_101111);
        self.pow2k_assign(2);
        self.mul_assign(&t_11);
        self.pow2k_assign(5);
        self.mul_assign(&t_11);
        self.pow2k_assign(5);
        self.mul_assign(&t_11);
        self.pow2k_assign(3);
        self.mul_assign(&t_1);
        self.pow2k_assign(7);
        self.mul_assign(&t_10101);
        self.pow2k_assign(6);
        self.mul_assign(&t_1111);
    }

    #[inline(always)]
    fn pow2k_assign(&mut self, k: usize) {
        for _ in 0..k {
            self.square_assign();
        }
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
