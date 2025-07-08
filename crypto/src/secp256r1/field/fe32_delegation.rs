use crate::{
    ark_ff_delegation::BigInt,
    bigint_delegation::{u256, DelegatedModParams, DelegatedMontParams},
};
use core::mem::MaybeUninit;
use core::ops::{AddAssign, MulAssign, SubAssign};

#[derive(Clone, Copy, Default)]
pub struct FieldElement(pub(super) BigInt<4>);

impl core::fmt::Debug for FieldElement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("0x")?;
        let bytes = self.to_be_bytes();
        for b in bytes.as_slice().iter() {
            f.write_fmt(format_args!("{:02x}", b))?;
        }
        core::fmt::Result::Ok(())
    }
}

static mut REDUCTION_CONST: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();
static mut MODULUS: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();
static mut R2: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();

pub fn init() {
    unsafe {
        REDUCTION_CONST.write(BigInt::<4>(super::REDUCTION_CONST));
        MODULUS.write(BigInt::<4>(super::MODULUS));
        R2.write(BigInt::<4>(super::R2));
    }
}

#[derive(Default, Debug)]
pub struct FieldParams;

impl DelegatedModParams<4> for FieldParams {
    unsafe fn modulus() -> &'static BigInt<4> {
        MODULUS.assume_init_ref()
    }
}

impl DelegatedMontParams<4> for FieldParams {
    unsafe fn reduction_const() -> &'static BigInt<4> {
        REDUCTION_CONST.assume_init_ref()
    }
}

impl FieldElement {
    pub(crate) const ZERO: Self = Self::from_words_unchecked([0; 4]);
    // montgomerry form
    pub(crate) const ONE: Self =
        Self::from_words_unchecked([1, 18446744069414584320, 18446744073709551615, 4294967294]);

    pub(super) fn to_representation(mut self) -> Self {
        unsafe {
            u256::mul_assign_montgomery::<FieldParams>(&mut self.0, R2.assume_init_ref());
        }
        self
    }

    pub(super) fn to_integer(mut self) -> Self {
        unsafe {
            u256::mul_assign_montgomery::<FieldParams>(&mut self.0, &BigInt::one());
        }
        self
    }

    pub(crate) const fn from_be_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        FieldElement(u256::from_bytes_unchecked(bytes))
    }

    pub(crate) const fn from_words_unchecked(words: [u64; 4]) -> Self {
        Self(BigInt::<4>(words))
    }

    pub(crate) fn from_words(words: [u64; 4]) -> Self {
        Self::from_words_unchecked(words).to_representation()
    }

    pub(crate) fn to_be_bytes(self) -> [u8; 32] {
        u256::to_be_bytes(self.to_integer().0)
    }

    pub(crate) fn is_zero(&self) -> bool {
        u256::is_zero(&self.0)
    }

    pub(crate) fn overflow(&self) -> bool {
        let modulus = unsafe { MODULUS.assume_init_ref() };
        !u256::lt(&self.0, modulus)
    }

    pub(crate) fn square_assign(&mut self) {
        unsafe {
            u256::square_assign_montgomery::<FieldParams>(&mut self.0);
        }
    }

    pub(crate) fn negate_assign(&mut self) {
        unsafe {
            u256::neg_mod_assign::<FieldParams>(&mut self.0);
        }
    }

    pub(crate) fn double_assign(&mut self) {
        unsafe {
            u256::double_mod_assign::<FieldParams>(&mut self.0);
        }
    }

    /// Computes `self = other - self`
    pub(crate) fn sub_and_negate_assign(&mut self, other: &Self) {
        unsafe {
            let borrow = u256::sub_and_negate_assign(&mut self.0, &other.0);
            if borrow {
                u256::add_assign(&mut self.0, FieldParams::modulus());
            }
        }
    }
}

impl AddAssign<&Self> for FieldElement {
    fn add_assign(&mut self, rhs: &Self) {
        unsafe {
            u256::add_mod_assign::<FieldParams>(&mut self.0, &rhs.0);
        }
    }
}

impl SubAssign<&Self> for FieldElement {
    fn sub_assign(&mut self, rhs: &Self) {
        unsafe {
            u256::sub_mod_assign::<FieldParams>(&mut self.0, &rhs.0);
        }
    }
}

impl MulAssign<&Self> for FieldElement {
    fn mul_assign(&mut self, rhs: &Self) {
        unsafe {
            u256::mul_assign_montgomery::<FieldParams>(&mut self.0, &rhs.0);
        }
    }
}

impl MulAssign<u32> for FieldElement {
    fn mul_assign(&mut self, rhs: u32) {
        let rhs = Self::from_words([rhs as u64, 0, 0, 0]);
        unsafe {
            u256::mul_assign_montgomery::<FieldParams>(&mut self.0, &rhs.0);
        }
    }
}

impl PartialEq for FieldElement {
    fn eq(&self, other: &Self) -> bool {
        u256::eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl proptest::arbitrary::Arbitrary for FieldElement {
        type Parameters = ();

        fn arbitrary_with(args: Self::Parameters) -> Self::Strategy {
            use proptest::prelude::{any, Strategy};

            any::<u256::U256Wrapper<FieldParams>>().prop_map(|x| Self(x.0).to_representation())
        }

        type Strategy = proptest::arbitrary::Mapped<u256::U256Wrapper<FieldParams>, FieldElement>;
    }
}
