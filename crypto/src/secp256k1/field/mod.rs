use crate::k256::FieldBytes;
use cfg_if::cfg_if;
use core::ops::{AddAssign, MulAssign, SubAssign};

#[cfg(any(target_arch = "riscv32", test))]
mod field_10x26;
#[cfg(any(target_arch = "riscv32", test))]
mod mod_inv32;

#[cfg(any(target_pointer_width = "64", test))]
mod field_5x52;
#[cfg(any(target_pointer_width = "64", test))]
mod mod_inv64;

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
mod field_8x32;
#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub use field_8x32::init;

#[cfg(all(debug_assertions, not(feature = "bigint_ops")))]
mod field_impl;

cfg_if! {
    if #[cfg(all(debug_assertions, not(feature = "bigint_ops")))] {
        use field_impl::{FieldElementImpl as FieldElementImplConst, FieldElementImpl, FieldStorageImpl};
    } else if #[cfg(feature = "bigint_ops")] {
        use field_10x26::{FieldElement10x26 as FieldElementImplConst, FieldStorage10x26 as FieldStorageImpl};
        use field_8x32::FieldElement8x32 as FieldElementImpl;
    } else if #[cfg(target_pointer_width = "64")] {
        use field_5x52::{FieldElement5x52 as FieldElementImpl, FieldElement5x52 as FieldElementImplConst, FieldStorage5x52 as FieldStorageImpl};
    } else if #[cfg(target_pointer_width = "32")] {
        use field_10x26::{FieldElement10x26 as FieldElementImplConst, FieldElement10x26 as FieldElementImpl, FieldStorage10x26 as FieldStorageImpl};
    } else {
        panic!("unsupported arch");
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FieldElementConst(pub(crate) FieldElementImplConst);

impl FieldElementConst {
    pub(crate) const ZERO: Self = Self(FieldElementImplConst::ZERO);
    pub(crate) const ONE: Self = Self(FieldElementImplConst::ONE);

    pub(crate) const fn from_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self(FieldElementImplConst::from_bytes_unchecked(bytes))
    }

    pub(crate) const fn mul(&self, rhs: &Self) -> Self {
        Self(self.0.mul(&rhs.0))
    }

    pub(crate) const fn mul_int(&self, rhs: u32) -> Self {
        Self(self.0.mul_int(rhs))
    }

    pub(crate) const fn square(&self) -> Self {
        Self(self.0.square())
    }

    pub(crate) const fn add(&self, rhs: &Self) -> Self {
        Self(self.0.add(&rhs.0))
    }

    pub(crate) const fn invert(&self) -> Self {
        let x2 = self.pow2k(1).mul(self);
        let x3 = x2.pow2k(1).mul(self);
        let x6 = x3.pow2k(3).mul(&x3);
        let x9 = x6.pow2k(3).mul(&x3);
        let x11 = x9.pow2k(2).mul(&x2);
        let x22 = x11.pow2k(11).mul(&x11);
        let x44 = x22.pow2k(22).mul(&x22);
        let x88 = x44.pow2k(44).mul(&x44);
        let x176 = x88.pow2k(88).mul(&x88);
        let x220 = x176.pow2k(44).mul(&x44);
        let x223 = x220.pow2k(3).mul(&x3);

        x223.pow2k(23)
            .mul(&x22)
            .pow2k(5)
            .mul(self)
            .pow2k(3)
            .mul(&x2)
            .pow2k(2)
            .mul(self)
    }

    pub(crate) const fn negate(&self, magnitude: u32) -> Self {
        Self(self.0.negate(magnitude))
    }

    pub(crate) const fn normalize(&self) -> Self {
        Self(self.0.normalize())
    }

    pub(crate) const fn to_storage(self) -> FieldStorage {
        FieldStorage(self.0.to_storage())
    }

    pub(crate) const fn normalizes_to_zero(&self) -> bool {
        self.0.normalizes_to_zero()
    }

    #[inline(always)]
    const fn pow2k(&self, k: usize) -> Self {
        use const_for::const_for;
        let mut x = *self;
        const_for!(_ in 0..k => {
            x = x.square();
        });

        x
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FieldElement(pub(crate) FieldElementImpl);

impl FieldElement {
    pub(crate) const ZERO: Self = Self(FieldElementImpl::ZERO);
    pub(crate) const ONE: Self = Self(FieldElementImpl::ONE);
    // 0x7ae96a2b657c07106e64479eac3434e99cf0497512f58995c1396c28719501ee
    pub(crate) const BETA: Self = Self(FieldElementImpl::BETA);

    #[cfg(test)]
    pub(crate) const fn from_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self(FieldElementImpl::from_bytes_unchecked(bytes))
    }

    pub(crate) fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        FieldElementImpl::from_bytes(bytes).map(Self)
    }

    pub(crate) fn mul_in_place(&mut self, rhs: &Self) {
        self.0.mul_in_place(&rhs.0);
    }

    pub(crate) fn mul_int_in_place(&mut self, rhs: u32) {
        self.0.mul_int_in_place(rhs);
    }

    pub(crate) fn square_in_place(&mut self) {
        self.0.square_in_place();
    }

    pub(crate) fn add_in_place(&mut self, rhs: &Self) {
        self.0.add_in_place(&rhs.0);
    }

    pub(crate) fn double_in_place(&mut self) {
        self.0.double_in_place();
    }

    pub(crate) fn sub_in_place(&mut self, rhs: &Self) {
        self.0.sub_in_place(&rhs.0);
    }

    pub(crate) fn add_int_in_place(&mut self, rhs: u32) {
        self.0.add_int_in_place(rhs);
    }

    pub(crate) fn invert_in_place(&mut self) {
        self.0.invert_in_place()
    }

    pub(crate) fn sqrt_in_place_unchecked(&mut self) {
        let x1 = *self;

        self.pow2k_in_place(1);
        *self *= x1;
        let x2 = *self;

        self.pow2k_in_place(1);
        *self *= x1;
        let x3 = *self;

        self.pow2k_in_place(3);
        *self *= x3;

        self.pow2k_in_place(3);
        *self *= x3;

        self.pow2k_in_place(2);
        *self *= x2;
        let x11 = *self;

        self.pow2k_in_place(11);
        *self *= x11;
        let x22 = *self;

        self.pow2k_in_place(22);
        *self *= x22;
        let x44 = *self;

        self.pow2k_in_place(44);
        *self *= x44;
        let x88 = *self;

        self.pow2k_in_place(88);
        *self *= x88;

        self.pow2k_in_place(44);
        *self *= x44;

        self.pow2k_in_place(3);
        *self *= x3;

        self.pow2k_in_place(23);
        *self *= x22;
        self.pow2k_in_place(6);
        *self *= x2;
        self.pow2k_in_place(2);
    }

    pub(crate) fn sqrt_in_place(&mut self) -> bool {
        let original = *self;
        self.sqrt_in_place_unchecked();

        let mut is_root = *self;
        is_root.square_in_place();
        is_root.negate_in_place(1);
        is_root.add_in_place(&original);

        is_root.normalizes_to_zero()
    }
    pub(crate) fn negate_in_place(&mut self, magnitude: u32) {
        self.0.negate_in_place(magnitude);
    }

    pub(crate) fn normalize_in_place(&mut self) {
        self.0.normalize_in_place();
    }

    pub(crate) fn is_odd(&self) -> bool {
        self.0.is_odd()
    }

    pub(crate) fn normalizes_to_zero(&self) -> bool {
        self.0.normalizes_to_zero()
    }

    #[inline(always)]
    fn pow2k_in_place(&mut self, k: usize) {
        for _ in 0..k {
            self.square_in_place();
        }
    }

    pub(crate) fn to_bytes(mut self) -> FieldBytes {
        self.normalize_in_place();
        self.0.to_bytes()
    }

    #[cfg(test)]
    pub(crate) const fn to_storage(self) -> FieldStorage {
        FieldStorage(self.0.to_storage())
    }
}

impl MulAssign for FieldElement {
    fn mul_assign(&mut self, rhs: Self) {
        self.mul_in_place(&rhs);
    }
}

impl MulAssign<&FieldElement> for FieldElement {
    fn mul_assign(&mut self, rhs: &Self) {
        self.mul_in_place(rhs);
    }
}

impl MulAssign<u32> for FieldElement {
    fn mul_assign(&mut self, rhs: u32) {
        self.mul_int_in_place(rhs);
    }
}

impl AddAssign for FieldElement {
    fn add_assign(&mut self, rhs: Self) {
        self.add_in_place(&rhs);
    }
}

impl AddAssign<u32> for FieldElement {
    fn add_assign(&mut self, rhs: u32) {
        self.add_int_in_place(rhs);
    }
}

impl SubAssign for FieldElement {
    fn sub_assign(&mut self, rhs: Self) {
        self.sub_in_place(&rhs);
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldStorage(FieldStorageImpl);

impl FieldStorage {
    pub(crate) const DEFAULT: Self = Self(FieldStorageImpl::DEFAULT);

    pub(crate) fn to_field_elem(self) -> FieldElement {
        FieldElement(self.0.to_field_elem())
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for FieldElementConst {
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<FieldElementImplConst>().prop_map(Self)
    }

    type Strategy = proptest::arbitrary::Mapped<FieldElementImplConst, Self>;
}

#[cfg(test)]
impl PartialEq for FieldElementConst {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for FieldElement {
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<FieldElementImpl>().prop_map(Self)
    }

    type Strategy = proptest::arbitrary::Mapped<FieldElementImpl, Self>;
}

#[cfg(test)]
impl PartialEq for FieldElement {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
mod tests {
    use super::{FieldElement, FieldElementConst};
    use proptest::{prop_assert, prop_assert_eq, proptest};

    #[test]
    fn storage_round_trip() {
        #[cfg(feature = "bigint_ops")]
        super::field_8x32::init();

        proptest!(|(x: FieldElement)| {
             prop_assert_eq!(x.to_storage().to_field_elem(), x);
        })
    }

    #[test]
    fn to_bytes_round_trip() {
        #[cfg(feature = "bigint_ops")]
        super::field_8x32::init();

        proptest!(|(x: FieldElement)| {
            let bytes: [u8; 32] = x.to_bytes().as_slice().try_into().unwrap();
            prop_assert_eq!(
                FieldElement::from_bytes(&bytes),
                Some(x)
            )
        });
    }

    #[test]
    fn from_bytes_round_trip() {
        #[cfg(feature = "bigint_ops")]
        super::field_8x32::init();

        proptest!(|(bytes: [u8; 32])| {
            prop_assert_eq!(
                &*FieldElement::from_bytes(&bytes).unwrap().to_bytes(),
                bytes
            )
        })
    }

    #[test]
    fn test_mul() {
        #[cfg(feature = "bigint_ops")]
        super::field_8x32::init();

        proptest!(|(x: FieldElement, y: FieldElement, z: FieldElement)| {
            let mut a = x;
            let mut b = y;
            a.mul_in_place(&y);
            b.mul_in_place(&x);
            prop_assert_eq!(a, b);

            a = x;
            b = y;
            a.mul_in_place(&y);
            a.mul_in_place(&z);
            b.mul_in_place(&z);
            b.mul_in_place(&x);
            prop_assert_eq!(a, b);

            a = x;
            a.mul_in_place(&FieldElement::ONE);
            prop_assert_eq!(a, x);

            a.mul_in_place(&FieldElement::ZERO);
            prop_assert_eq!(a, FieldElement::ZERO);

            a = y;
            a.add_in_place(&z);
            a.mul_in_place(&x);

            b = x;
            let mut c = x;
            b.mul_in_place(&y);
            c.mul_in_place(&z);
            b.add_in_place(&c);
            prop_assert_eq!(a, b);
        });
    }

    #[test]
    fn test_mul_const() {
        proptest!(|(x: FieldElementConst, y: FieldElementConst, z: FieldElementConst)| {
            prop_assert_eq!(x.mul(&y), y.mul(&x));
            prop_assert_eq!(x.mul(&y).mul(&z), x.mul(&y.mul(&z)));
            prop_assert_eq!(x.mul(&FieldElementConst::ONE), x);
            prop_assert_eq!(x.mul(&FieldElementConst::ZERO), FieldElementConst::ZERO);

            prop_assert_eq!(x.mul(&y.add(&z)), x.mul(&y).add(&x.mul(&z)));
        });
    }

    #[test]
    fn test_invert() {
        #[cfg(feature = "bigint_ops")]
        super::field_8x32::init();

        proptest!(|(x: FieldElement)| {
            let mut a = x;
            a.invert_in_place();
            a.invert_in_place();
            prop_assert_eq!(a, x);

            a.invert_in_place();
            a.mul_in_place(&x);
            if x.normalizes_to_zero() {
                prop_assert_eq!(a, FieldElement::ZERO);
            } else {
                prop_assert_eq!(a, FieldElement::ONE);
            }
        })
    }

    #[test]
    fn test_invert_const() {
        proptest!(|(x: FieldElementConst)| {
            prop_assert_eq!(x.invert().invert(), x);

            if x.normalizes_to_zero() {
                prop_assert_eq!(x.invert().mul(&x), FieldElementConst::ZERO);
            } else {
                prop_assert_eq!(x.invert().mul(&x), FieldElementConst::ONE);
            }
        })
    }

    #[test]
    fn test_add() {
        #[cfg(feature = "bigint_ops")]
        super::field_8x32::init();

        proptest!(|(x: FieldElement, y: FieldElement, z: FieldElement)| {
            let mut a = x;
            let mut b = y;
            a.add_in_place(&y);
            b.add_in_place(&x);
            prop_assert_eq!(a, b);

            a = x;
            a.add_in_place(&FieldElement::ZERO);
            prop_assert_eq!(a, x);

            b = y;
            a.add_in_place(&y);
            a.add_in_place(&z);
            b.add_in_place(&z);
            b.add_in_place(&x);
            prop_assert_eq!(a, b);

            a = x;
            a.negate_in_place(1);
            a.add_in_place(&x);
            prop_assert_eq!(a, FieldElement::ZERO);

            a = x;
            a.sub_in_place(&x);
            prop_assert_eq!(a, FieldElement::ZERO);
        });
    }

    #[test]
    fn test_add_const() {
        proptest!(|(x: FieldElementConst, y: FieldElementConst, z: FieldElementConst)| {
            prop_assert_eq!(x.add(&y), y.add(&x));
            prop_assert_eq!(x.add(&FieldElementConst::ZERO), x);
            prop_assert_eq!(x.add(&y).add(&z), x.add(&y.add(&z)));
            prop_assert_eq!(x.add(&x.negate(1)), FieldElementConst::ZERO);
        });
    }

    #[test]
    fn test_square() {
        #[cfg(feature = "bigint_ops")]
        super::field_8x32::init();

        proptest!(|(x: FieldElement)| {
            let mut x_neg = x;
            x_neg.negate_in_place(1);

            let mut a = x;
            let mut b = x;
            a.square_in_place();
            b.mul_in_place(&x);
            prop_assert_eq!(a, b);

            a = x;
            a.square_in_place();
            a.sqrt_in_place();

            prop_assert!(a == x || a == x_neg);

            a = x;
            a.sqrt_in_place();
            a.square_in_place();

            prop_assert!(a == x || a == x_neg)
        });
    }

    #[test]
    fn test_square_const() {
        proptest!(|(x: FieldElementConst)| {
            prop_assert_eq!(x.square(), x.mul(&x));
        })
    }
}
