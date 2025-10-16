use core::mem::MaybeUninit;

use crate::ark_ff_delegation::BigInt;
use crate::bigint_delegation::{u256, DelegatedModParams, DelegatedMontParams};
use crate::secp256r1::Secp256r1Err;

static mut MODULUS: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();
static mut REDUCTION_CONST: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();
static mut R2: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();

pub(crate) fn init() {
    unsafe {
        MODULUS.write(BigInt::<4>(super::MODULUS));
        REDUCTION_CONST.write(BigInt::<4>(super::REDUCTION_CONST));
        R2.write(BigInt::<4>(super::R2));
    }
}

#[derive(Default, Debug)]
pub struct ScalarParams;

impl DelegatedModParams<4> for ScalarParams {
    unsafe fn modulus() -> &'static BigInt<4> {
        MODULUS.assume_init_ref()
    }
}

impl DelegatedMontParams<4> for ScalarParams {
    unsafe fn reduction_const() -> &'static BigInt<4> {
        REDUCTION_CONST.assume_init_ref()
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Scalar(BigInt<4>);

impl Scalar {
    pub(crate) const ZERO: Self = Self(BigInt::zero());
    // montgomerry form
    pub(crate) const ONE: Self = Self(BigInt::<4>([
        884452912994769583,
        4834901526196019579,
        0,
        4294967295,
    ]));

    pub(super) fn to_repressentation(mut self) -> Self {
        unsafe {
            u256::mul_assign_montgomery::<ScalarParams>(&mut self.0, R2.assume_init_ref());
        }
        self
    }

    pub(super) fn to_integer(mut self) -> Self {
        unsafe {
            u256::mul_assign_montgomery::<ScalarParams>(&mut self.0, &BigInt::one());
        }
        self
    }

    pub(crate) fn reduce_be_bytes(bytes: &[u8; 32]) -> Self {
        Self::from_be_bytes_unchecked(bytes).to_repressentation()
    }

    pub(super) fn from_be_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self(u256::from_bytes_unchecked(bytes))
    }

    pub(crate) fn from_be_bytes(bytes: &[u8; 32]) -> Result<Self, Secp256r1Err> {
        let val = Self::from_be_bytes_unchecked(bytes);
        if val.overflow() {
            Err(Secp256r1Err::InvalidSignature)
        } else {
            Ok(val.to_repressentation())
        }
    }

    fn overflow(&self) -> bool {
        let mut temp = *self;
        let borrow = u256::sub_and_negate_assign(&mut temp.0, unsafe { MODULUS.assume_init_ref() });
        borrow
    }

    pub(crate) fn from_words(words: [u64; 4]) -> Self {
        Self(BigInt::<4>(words)).to_repressentation()
    }

    pub(super) fn to_words(self) -> [u64; 4] {
        self.to_integer().0 .0
    }

    pub(crate) fn is_zero(&self) -> bool {
        u256::is_zero(&self.0)
    }

    pub(super) fn square_assign(&mut self) {
        unsafe {
            u256::square_assign_montgomery::<ScalarParams>(&mut self.0);
        }
    }

    pub(super) fn mul_assign(&mut self, rhs: &Self) {
        unsafe {
            u256::mul_assign_montgomery::<ScalarParams>(&mut self.0, &rhs.0);
        }
    }

    pub(super) fn neg_assign(&mut self) {
        unsafe {
            u256::neg_mod_assign::<ScalarParams>(&mut self.0);
        }
    }

    pub(super) fn eq_inner(&self, other: &Self) -> bool {
        u256::eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{u256, Scalar, ScalarParams};

    impl proptest::arbitrary::Arbitrary for Scalar {
        type Parameters = ();

        fn arbitrary_with(args: Self::Parameters) -> Self::Strategy {
            use proptest::prelude::{any, Strategy};
            any::<u256::U256Wrapper<ScalarParams>>().prop_map(|x| Self(x.0).to_repressentation())
        }

        type Strategy = proptest::arbitrary::Mapped<u256::U256Wrapper<ScalarParams>, Scalar>;
    }
}
