use core::ops::{AddAssign, MulAssign};

use crate::k256::FieldBytes;
use cfg_if::cfg_if;

mod invert;

#[cfg(all(target_pointer_width = "64", not(feature = "bigint_ops")))]
mod scalar64;

#[cfg(all(target_pointer_width = "32", not(feature = "bigint_ops")))]
mod scalar32;

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub(crate) mod scalar32_delegation;
#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub use scalar32_delegation::init;

cfg_if! {
    if #[cfg(feature = "bigint_ops")] {
        use scalar32_delegation::ScalarInner;
    } else if #[cfg(target_pointer_width = "32")] {
        use scalar32::ScalarInner;
    } else if #[cfg(target_pointer_width = "64")] {
        use scalar64::ScalarInner;
    }
}

const ORDER_HEX: &str = "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141";

#[derive(Debug, Clone, Copy)]
pub struct Scalar(pub(crate) ScalarInner);

impl Scalar {
    #[cfg(test)]
    pub(crate) const ZERO: Self = Self(ScalarInner::ZERO);
    #[cfg(test)]
    pub(crate) const ONE: Self = Self(ScalarInner::ONE);
    #[cfg(test)]
    const ORDER: Self = Self(ScalarInner::ORDER);
    #[cfg(test)]
    const MINUS_LAMBDA: Self = Self(ScalarInner::MINUS_LAMBDA);

    #[cfg(test)]
    pub(crate) const fn from_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self(ScalarInner::from_be_bytes_unchecked(bytes))
    }

    #[cfg(test)]
    pub(crate) fn from_u128(n: u128) -> Self {
        Self(ScalarInner::from_u128(n))
    }

    #[cfg(test)]
    pub(crate) fn from_be_hex(hex: &str) -> Self {
        Self(ScalarInner::from_be_hex(hex))
    }

    pub(crate) fn from_signature(signature: &crate::k256::ecdsa::Signature) -> (Self, Self) {
        let (r, s) = signature.split_scalars();
        (Self::from_k256_scalar(*r), Self::from_k256_scalar(*s))
    }

    pub(crate) fn to_repr(self) -> FieldBytes {
        self.0.to_be_bytes().into()
    }

    #[cfg(test)]
    pub(crate) fn from_repr(bytes: FieldBytes) -> Self {
        let bytes = bytes.as_slice().try_into().unwrap();
        Self(ScalarInner::from_be_bytes(bytes))
    }

    #[inline(always)]
    pub(crate) fn from_k256_scalar(s: crate::k256::Scalar) -> Self {
        Self(ScalarInner::from_k256_scalar(s))
    }

    pub(crate) fn decompose(self) -> (Self, Self) {
        let (k1, k2) = self.0.decompose();
        (Self(k1), Self(k2))
    }

    pub(crate) fn decompose_128(self) -> (Self, Self) {
        let (k1, k2) = self.0.decompose_128();
        (Self(k1), Self(k2))
    }

    pub(crate) fn bits(&self, offset: usize, count: usize) -> u32 {
        self.0.bits(offset, count)
    }

    pub(crate) fn bits_var(&self, offset: usize, count: usize) -> u32 {
        self.0.bits_var(offset, count)
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub(crate) fn negate_in_place(&mut self) {
        self.0.negate_in_place();
    }
}

impl MulAssign for Scalar {
    fn mul_assign(&mut self, rhs: Self) {
        self.0.mul_in_place(&rhs.0);
    }
}

impl MulAssign<&Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: &Scalar) {
        self.0.mul_in_place(&rhs.0);
    }
}

impl AddAssign for Scalar {
    fn add_assign(&mut self, rhs: Self) {
        self.0.add_in_place(&rhs.0);
    }
}

impl AddAssign<&Scalar> for Scalar {
    fn add_assign(&mut self, rhs: &Scalar) {
        self.0.add_in_place(&rhs.0);
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for Scalar {
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<ScalarInner>().prop_map(|inner| Self(inner))
    }

    type Strategy = proptest::arbitrary::Mapped<ScalarInner, Self>;
}

#[cfg(test)]
impl PartialEq for Scalar {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
impl PartialOrd for Scalar {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

#[cfg(test)]
impl core::ops::Neg for Scalar {
    type Output = Self;

    fn neg(self) -> Self::Output {
        let mut x = self;
        x.0.negate_in_place();
        x
    }
}

#[cfg(test)]
impl core::ops::Mul for Scalar {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut lhs = self;
        lhs.0.mul_in_place(&rhs.0);
        lhs
    }
}

#[cfg(test)]
impl core::ops::Add for Scalar {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut lhs = self;
        lhs.0.add_in_place(&rhs.0);
        lhs
    }
}

#[cfg(test)]
impl core::ops::Sub for Scalar {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::Scalar;
    use proptest::{prop_assert, prop_assert_eq, proptest};

    fn init() {
        #[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
        super::scalar32_delegation::init();
    }

    #[test]
    fn test_zero() {
        init();

        assert_eq!(Scalar::ZERO, Scalar::ORDER);
        assert!(Scalar::ZERO.is_zero());
        assert!(Scalar::ORDER.is_zero());
    }

    #[test]
    fn test_mul() {
        init();

        proptest!(|(x: Scalar, y: Scalar, z: Scalar)| {
            prop_assert_eq!(x * y, y * x);
            prop_assert_eq!((x * y) * z, x * (y * z));
            prop_assert_eq!(x * Scalar::ONE, x);
            prop_assert_eq!(x * Scalar::ZERO, Scalar::ZERO);

            prop_assert_eq!(x * (y + z), (x * y) + (x * z));
        })
    }

    #[test]
    fn test_add() {
        init();

        proptest!(|(x: Scalar, y: Scalar, z: Scalar)| {
            prop_assert_eq!(x + y, y + x);
            prop_assert_eq!(x + Scalar::ZERO, x);
            prop_assert_eq!(x + (y + z), (x + y) + z);
            prop_assert_eq!(x - x, Scalar::ZERO);
        })
    }

    #[test]
    fn test_decompose() {
        init();

        proptest!(|(k: Scalar)| {
            let (mut r1, mut r2) = k.decompose();
            let lambda = -Scalar::MINUS_LAMBDA;

            #[cfg(feature = "bigint_ops")]
            {
                r1 = Scalar(r1.0.to_representation());
                r2 = Scalar(r2.0.to_representation());
            }

            prop_assert_eq!(r1 + r2 * lambda, k);

            #[cfg(feature = "bigint_ops")]
            {
                r1 = Scalar(r1.0.to_integer());
                r2 = Scalar(r2.0.to_integer());
            }

            let bound = Scalar::from_bytes_unchecked(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            prop_assert!(r1 < bound || -r1 < bound);
            prop_assert!(r2 < bound || -r2 < bound);
        })
    }
}
