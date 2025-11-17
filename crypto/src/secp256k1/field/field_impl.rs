// most of the code in this file comes from https://github.com/RustCrypto/elliptic-curves/blob/master/k256/src/arithmetic/field/field_impl.rs
use crate::k256::FieldBytes;
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(
        any(target_arch = "riscv32"),
        all(feature = "proving", fuzzing)
    )] {
        use super::field_10x26::{FieldElement10x26 as FieldElementInner, FieldStorage10x26 as FieldStorageInner};
    } else if #[cfg(target_pointer_width = "64")] {
        use super::field_5x52::{FieldElement5x52 as FieldElementInner, FieldStorage5x52 as FieldStorageInner};
    } else {
        panic!("unsupported target arch");
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FieldElementImpl {
    value: FieldElementInner,
    pub(crate) magnitude: u32,
    normalized: bool,
}

impl FieldElementImpl {
    pub(super) const ZERO: Self = Self::new_normalized(FieldElementInner::ZERO);
    pub(super) const ONE: Self = Self::new_normalized(FieldElementInner::ONE);
    pub(super) const BETA: Self = Self::new_normalized(FieldElementInner::BETA);

    const fn new(value: FieldElementInner, magnitude: u32) -> Self {
        debug_assert!(magnitude <= Self::max_magnitude());
        Self {
            value,
            magnitude,
            normalized: false,
        }
    }

    const fn new_normalized(value: FieldElementInner) -> Self {
        Self {
            value,
            magnitude: 1,
            normalized: true,
        }
    }

    const fn new_weak_normalized(value: FieldElementInner) -> Self {
        Self {
            value,
            magnitude: 1,
            normalized: false,
        }
    }

    pub(super) const fn from_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        let value = FieldElementInner::from_bytes_unchecked(bytes);
        Self::new_normalized(value)
    }

    pub(super) fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        FieldElementInner::from_bytes(bytes).map(Self::new_normalized)
    }

    pub(super) fn to_bytes(self) -> FieldBytes {
        self.value.to_bytes()
    }

    const fn max_magnitude() -> u32 {
        FieldElementInner::max_magnitude()
    }

    pub(super) fn mul_in_place(&mut self, rhs: &Self) {
        debug_assert!(self.magnitude <= 8);
        debug_assert!(rhs.magnitude <= 8);

        self.value.mul_in_place(&rhs.value);
        self.magnitude = 1;
        self.normalized = false;
    }

    pub(super) fn mul_int_in_place(&mut self, rhs: u32) {
        self.magnitude *= rhs;
        debug_assert!(self.magnitude <= Self::max_magnitude());

        self.value.mul_int_in_place(rhs);
        self.normalized = false;
    }

    pub(super) fn square_in_place(&mut self) {
        debug_assert!(self.magnitude <= 8);
        self.value.square_in_place();
        self.magnitude = 1;
        self.normalized = false;
    }

    pub(super) fn add_in_place(&mut self, rhs: &Self) {
        self.magnitude += rhs.magnitude;
        debug_assert!(self.magnitude <= Self::max_magnitude());

        self.value.add_in_place(&rhs.value);
        self.normalized = false;
    }

    pub(super) fn double_in_place(&mut self) {
        self.magnitude *= 2;
        self.normalized = false;
        self.value.double_in_place()
    }

    pub(super) fn sub_in_place(&mut self, rhs: &Self) {
        self.magnitude += 1;
        self.value.sub_in_place(&rhs.value);
        self.normalized = false;
    }

    pub(super) fn add_int_in_place(&mut self, rhs: u32) {
        self.magnitude += rhs;
        debug_assert!(self.magnitude <= Self::max_magnitude());

        self.value.add_int_in_place(rhs);
        self.normalized = false;
    }

    pub(super) fn negate_in_place(&mut self, magnitude: u32) {
        debug_assert!(self.magnitude <= magnitude);
        self.magnitude = magnitude + 1;
        self.value.negate_in_place(magnitude);
        self.normalized = false;
    }

    pub(super) fn normalize_in_place(&mut self) {
        if !self.normalized || self.magnitude > 1 {
            self.value.normalize_in_place();
            self.magnitude = 1;
            self.normalized = true;
        }
    }

    pub(super) const fn normalizes_to_zero(&self) -> bool {
        self.value.normalizes_to_zero()
    }

    pub(super) fn invert_in_place(&mut self) {
        self.value.invert_in_place();
        self.magnitude = 1;
        self.normalized = true;
    }

    pub(super) fn is_odd(&self) -> bool {
        self.value.is_odd()
    }

    pub(super) const fn mul(&self, rhs: &Self) -> Self {
        debug_assert!(self.magnitude <= 8);
        debug_assert!(rhs.magnitude <= 8);

        let res = self.value.mul(&rhs.value);
        Self::new_weak_normalized(res)
    }

    pub(super) const fn mul_int(&self, rhs: u32) -> Self {
        let new_magnitude = self.magnitude * rhs;
        debug_assert!(new_magnitude <= Self::max_magnitude());

        let value = self.value.mul_int(rhs);
        Self::new(value, new_magnitude)
    }

    pub(super) const fn square(&self) -> Self {
        debug_assert!(self.magnitude <= 8);
        Self::new_weak_normalized(self.value.square())
    }

    pub(super) const fn add(&self, rhs: &Self) -> Self {
        let new_magnitude = self.magnitude + rhs.magnitude;
        debug_assert!(new_magnitude <= Self::max_magnitude());

        let value = self.value.add(&rhs.value);
        Self::new(value, new_magnitude)
    }

    pub(super) const fn negate(&self, magnitude: u32) -> Self {
        debug_assert!(self.magnitude <= magnitude);
        let new_magnitude = magnitude + 1;
        debug_assert!(new_magnitude <= Self::max_magnitude());

        let value = self.value.negate(magnitude);
        Self::new(value, new_magnitude)
    }

    pub(super) const fn normalize(self) -> Self {
        if self.normalized && self.magnitude <= 1 {
            self
        } else {
            Self::new_normalized(self.value.normalize())
        }
    }

    pub(super) const fn to_storage(self) -> FieldStorageImpl {
        FieldStorageImpl(self.value.to_storage())
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FieldStorageImpl(FieldStorageInner);

impl FieldStorageImpl {
    pub(super) const DEFAULT: Self = Self(FieldStorageInner::DEFAULT);

    pub(super) fn to_field_elem(self) -> FieldElementImpl {
        FieldElementImpl {
            value: self.0.to_field_elem(),
            magnitude: 1,
            normalized: true,
        }
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for FieldElementImpl {
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<FieldElementInner>().prop_map(|value| Self {
            value,
            magnitude: 1,
            normalized: true,
        })
    }

    type Strategy = proptest::arbitrary::Mapped<FieldElementInner, Self>;
}

#[cfg(test)]
impl PartialEq for FieldElementImpl {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

#[cfg(test)]
mod tests {
    use crate::secp256k1::field::field_impl::FieldElementImpl;

    #[test]
    fn test_magnitude_regression_mul_int() {
        // Regression test for the magnitude calculation bug fix

        // Test mul_int_in_place with correct magnitude tracking
        let mut fe = FieldElementImpl::ONE;
        fe.add_in_place(&FieldElementImpl::ONE);

        let initial_magnitude = fe.magnitude;
        assert_eq!(initial_magnitude, 2);

        let multiplier = 3u32;
        fe.mul_int_in_place(multiplier);

        assert_eq!(fe.magnitude, initial_magnitude * multiplier);
        assert_eq!(fe.magnitude, 6);

        // Test const mul_int with correct magnitude tracking
        let mut fe2 = FieldElementImpl::ONE;
        for _ in 0..4 {
            fe2.add_in_place(&FieldElementImpl::ONE);
        }
        assert_eq!(fe2.magnitude, 5);

        let multiplier2 = 4u32;
        let result = fe2.mul_int(multiplier2);

        assert_eq!(result.magnitude, fe2.magnitude * multiplier2);
        assert_eq!(result.magnitude, 20);

        // Test edge cases
        let mut fe3 = FieldElementImpl::ONE;
        fe3.add_in_place(&FieldElementImpl::ONE); // magnitude = 2

        let original_magnitude = fe3.magnitude;
        fe3.mul_int_in_place(1); // multiply by 1
        assert_eq!(fe3.magnitude, original_magnitude); // should preserve magnitude

        let mut fe4 = FieldElementImpl::ONE;
        fe4.add_in_place(&FieldElementImpl::ONE); // magnitude = 2
        fe4.mul_int_in_place(0); // multiply by 0
        assert_eq!(fe4.magnitude, 0); // magnitude should be 0
    }
}
