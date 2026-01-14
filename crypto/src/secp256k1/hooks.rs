//! This module defines hooks for Secp256k1 field operations that can be
//! overridden to provide custom implementations, such as using an oracle for
//! square root and inversion operations.

pub trait Secp256k1Hooks {
    /// Computes the square root of a Secp256k1 field element and assigns it to the input.
    /// Returns true if the square root exists, false otherwise.
    fn fe_sqrt_and_assign(&mut self, fe: &mut crate::secp256k1::field::FieldElement) -> bool;

    /// Computes the multiplicative inverse of a Secp256k1 field element and assigns it to the input.
    fn fe_invert_and_assign(&mut self, fe: &mut crate::secp256k1::field::FieldElement);

    /// Computes the multiplicative inverse of a Secp256k1 scalar and assigns it to the input.
    fn scalar_invert_and_assign(&mut self, scalar: &mut crate::secp256k1::scalars::Scalar);
}

pub struct DefaultSecp256k1Hooks;

impl Secp256k1Hooks for DefaultSecp256k1Hooks {
    #[inline(always)]
    fn fe_sqrt_and_assign(&mut self, fe: &mut crate::secp256k1::field::FieldElement) -> bool {
        fe.sqrt_in_place_inner()
    }

    #[inline(always)]
    fn fe_invert_and_assign(&mut self, fe: &mut crate::secp256k1::field::FieldElement) {
        fe.invert_in_place_inner()
    }

    #[inline(always)]
    fn scalar_invert_and_assign(&mut self, scalar: &mut crate::secp256k1::scalars::Scalar) {
        scalar.invert_in_place_inner()
    }
}
