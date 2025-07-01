use crate::ark_ff_delegation::BigIntMacro;
use crate::bigint_arithmatic::u256::*;
use crate::k256::FieldBytes;
use bigint_riscv::DelegatedU256;
use core::mem::MaybeUninit;

use super::field_10x26::FieldStorage10x26;

#[derive(Clone, Debug)]
pub(super) struct FieldElement8x32(pub(super) DelegatedU256);

static mut MODULUS: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();
static mut NEG_MODULUS: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();

#[derive(Debug, Default)]
pub(super) struct FieldParams;

impl DelegatedModParams for FieldParams {
    unsafe fn modulus() -> &'static DelegatedU256 {
        unsafe { MODULUS.assume_init_ref() }
    }
}

impl DelegatedBarretParams for FieldParams {
    unsafe fn neg_modulus() -> &'static DelegatedU256 {
        unsafe { NEG_MODULUS.assume_init_ref() }
    }
}

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub fn init() {
    unsafe {
        MODULUS.as_mut_ptr().write(FieldElement8x32::MODULUS);
        NEG_MODULUS
            .as_mut_ptr()
            .write(FieldElement8x32::NEG_MODULUS);
    }
}

impl FieldElement8x32 {
    pub(super) const ZERO: Self = Self(DelegatedU256::ZERO);
    pub(super) const BETA: Self = const {
        let limbs = BigIntMacro!(
            "55594575648329892869085402983802832744385952214688224221778511981742606582254"
        )
        .0;

        Self(DelegatedU256::from_limbs(limbs))
    };
    pub(super) const ONE: Self = Self(DelegatedU256::ONE);
    // 2^256 - MODULUS
    const NEG_MODULUS: DelegatedU256 = const {
        let limbs = BigIntMacro!("4294968273").0;

        DelegatedU256::from_limbs(limbs)
    };

    const MODULUS: DelegatedU256 = const {
        let limbs = BigIntMacro!(
            "115792089237316195423570985008687907853269984665640564039457584007908834671663"
        )
        .0;

        DelegatedU256::from_limbs(limbs)
    };

    #[inline(always)]
    pub(super) const fn from_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self(DelegatedU256::from_be_bytes(bytes))
    }

    #[inline(always)]
    pub(super) fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        let value = Self::from_bytes_unchecked(bytes);

        if value.0.le(&Self::MODULUS) {
            Some(value)
        } else {
            None
        }
    }

    #[inline(always)]
    pub(super) fn to_bytes(self) -> FieldBytes {
        self.0.to_be_bytes().into()
    }

    pub(super) fn from_words(words: [u64; 4]) -> Self {
        Self(DelegatedU256::from_limbs(words))
    }

    #[inline(always)]
    pub(super) fn mul_in_place(&mut self, rhs: &Self) {
        unsafe {
            mul_assign_barret::<FieldParams>(&mut self.0, &rhs.0);
        }
    }

    #[inline(always)]
    pub(super) fn mul_int_in_place(&mut self, rhs: u32) {
        let rhs = DelegatedU256::from_limbs([rhs as u64, 0, 0, 0]);
        unsafe {
            mul_assign_barret::<FieldParams>(&mut self.0, &rhs);
        }
    }

    #[inline(always)]
    pub(super) fn square_in_place(&mut self) {
        unsafe {
            square_assign_barret::<FieldParams>(&mut self.0);
        }
    }

    #[inline(always)]
    pub(super) fn add_int_in_place(&mut self, rhs: u32) {
        let rhs = DelegatedU256::from_limbs([rhs as u64, 0, 0, 0]);
        unsafe {
            add_mod_assign::<FieldParams>(&mut self.0, &rhs);
        }
    }

    #[inline(always)]
    pub(super) fn add_in_place(&mut self, rhs: &Self) {
        unsafe {
            add_mod_assign::<FieldParams>(&mut self.0, &rhs.0);
        }
    }

    #[inline(always)]
    pub(super) fn double_in_place(&mut self) {
        unsafe {
            double_mod_assign::<FieldParams>(&mut self.0);
        }
    }

    #[inline(always)]
    pub(super) fn sub_in_place(&mut self, rhs: &Self) {
        unsafe { sub_mod_assign::<FieldParams>(&mut self.0, &rhs.0) };
    }

    #[inline(always)]
    pub(super) fn negate_in_place(&mut self, _magnitude: u32) {
        unsafe { neg_mod_assign::<FieldParams>(&mut self.0) };
    }

    #[inline(always)]
    pub(super) fn normalize_in_place(&mut self) {
        // the 8x32 implementation is always normalized
    }

    #[inline(always)]
    pub(super) fn normalizes_to_zero(&self) -> bool {
        unsafe { self.0.is_zero() }
    }

    #[inline(always)]
    pub(super) fn is_odd(&self) -> bool {
        self.0.is_odd()
    }

    #[inline(always)]
    pub(super) const fn to_storage(self) -> FieldStorage10x26 {
        let mut res = [0; 8];
        let words = self.0.as_limbs();
        let mut i = 0;
        while i < 4 {
            res[2 * i] = words[i] as u32;
            res[2 * i + 1] = (words[i] >> 32) as u32;
            i += 1;
        }
        FieldStorage10x26(res)
    }

    #[inline(always)]
    fn pow2k_in_place(&mut self, k: usize) {
        for _ in 0..k {
            self.square_in_place();
        }
    }

    #[inline(always)]
    pub(super) fn invert_in_place(&mut self) {
        let x1 = self.clone();

        self.pow2k_in_place(1);
        self.mul_in_place(&x1);
        let x2 = self.clone();

        self.pow2k_in_place(1);
        self.mul_in_place(&x1);
        let x3 = self.clone();

        self.pow2k_in_place(3);
        self.mul_in_place(&x3);

        self.pow2k_in_place(3);
        self.mul_in_place(&x3);

        self.pow2k_in_place(2);
        self.mul_in_place(&x2);
        let x11 = self.clone();

        self.pow2k_in_place(11);
        self.mul_in_place(&x11);
        let x22 = self.clone();

        self.pow2k_in_place(22);
        self.mul_in_place(&x22);
        let x44 = self.clone();

        self.pow2k_in_place(44);
        self.mul_in_place(&x44);
        let x88 = self.clone();

        self.pow2k_in_place(88);
        self.mul_in_place(&x88);

        self.pow2k_in_place(44);
        self.mul_in_place(&x44);

        self.pow2k_in_place(3);
        self.mul_in_place(&x3);

        self.pow2k_in_place(23);
        self.mul_in_place(&x22);
        self.pow2k_in_place(5);
        self.mul_in_place(&x1);
        self.pow2k_in_place(3);
        self.mul_in_place(&x2);
        self.pow2k_in_place(2);
        self.mul_in_place(&x1);
    }
}

#[cfg(test)]
impl PartialEq for FieldElement8x32 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for FieldElement8x32 {
    type Parameters = ();

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<U256Wrapper<FieldParams>>().prop_map(|x| Self(x.0))
    }

    type Strategy = proptest::arbitrary::Mapped<U256Wrapper<FieldParams>, Self>;
}

#[cfg(test)]
mod tests {
    use super::FieldElement8x32;
    use proptest::{prop_assert_eq, proptest};

    fn init() {
        crate::secp256k1::init();
        bigint_riscv::init();
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_invert() {
        init();
        proptest!(|(x: FieldElement8x32)| {
            let mut a = x.clone();
            a.invert_in_place();
            a.invert_in_place();
            prop_assert_eq!(a.clone(), x.clone());

            a = x.clone();
            a.invert_in_place();
            a.mul_in_place(&x.clone());

            if x.normalizes_to_zero() {
                prop_assert_eq!(a, FieldElement8x32::ZERO);
            } else {
                prop_assert_eq!(a, FieldElement8x32::ONE);
            }
        })
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_mul() {
        init();
        proptest!(|(x: FieldElement8x32, y: FieldElement8x32, z: FieldElement8x32)| {
            let mut a = x.clone();
            let mut b = y.clone();

            // x * y = y * x
            a.mul_in_place(&y);
            b.mul_in_place(&x);
            prop_assert_eq!(a, b);

            // (x * y) * z = x * (y * z)
            a = x.clone();
            b = y.clone();
            a.mul_in_place(&y);
            a.mul_in_place(&z);
            b.mul_in_place(&z);
            b.mul_in_place(&x);
            prop_assert_eq!(a, b);

            // x * 1 = x
            a = x.clone();
            a.mul_in_place(&FieldElement8x32::ONE);
            prop_assert_eq!(a, x.clone());

            // x * 0 = 0
            a = x.clone();
            a.mul_in_place(&FieldElement8x32::ZERO);
            prop_assert_eq!(a, FieldElement8x32::ZERO);

            // x * (y + z) = x * y + x * z
            a = y.clone();
            b = x.clone();
            let mut c = x.clone();
            a.add_in_place(&z);
            a.mul_in_place(&x);
            b.mul_in_place(&y);
            c.mul_in_place(&z);
            b.add_in_place(&c);
            prop_assert_eq!(a, b);
        })
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_add() {
        init();
        proptest!(|(x: FieldElement8x32, y: FieldElement8x32, z: FieldElement8x32)| {
            let mut a = x.clone();
            let mut b = y.clone();

            // x + y = y + x
            a.add_in_place(&y);
            b.add_in_place(&x);
            prop_assert_eq!(a, b);

            // x + 0 = x
            a = x.clone();
            a.add_in_place(&FieldElement8x32::ZERO);
            prop_assert_eq!(a, x.clone());

            // (x + y) + z = x + (y + z)
            a = x.clone();
            b = y.clone();
            a.add_in_place(&y);
            a.add_in_place(&z);
            b.add_in_place(&z);
            b.add_in_place(&x);
            prop_assert_eq!(a, b);

            // x - x = 0
            a = x.clone();
            a.sub_in_place(&x);
            prop_assert_eq!(a, FieldElement8x32::ZERO);

            // x + y - y = x
            a = x.clone();
            a.add_in_place(&y);
            a.sub_in_place(&y);
            prop_assert_eq!(a, x.clone());

            // x - y + y = x
            a = x.clone();
            a.sub_in_place(&y);
            a.add_in_place(&y);
            prop_assert_eq!(a, x);
        })
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn from_bytes_round() {
        proptest!(|(bytes: [u8; 32])| {
            prop_assert_eq!(&*FieldElement8x32::from_bytes_unchecked(&bytes).to_bytes(), &bytes);
        })
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn to_bytes_round() {
        proptest!(|(x: FieldElement8x32)| {
            let bytes = &*x.clone().to_bytes();
            prop_assert_eq!(FieldElement8x32::from_bytes_unchecked(bytes.try_into().unwrap()), x);
        })
    }
}
