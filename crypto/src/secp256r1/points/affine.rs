use core::ops::Neg;

use crate::secp256r1::{field::FieldElement, Secp256r1Err};

use super::Jacobian;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Affine {
    pub(crate) x: FieldElement,
    pub(crate) y: FieldElement,
    pub(crate) infinity: bool,
}

impl Affine {
    pub(super) const INFINITY: Self = Self {
        x: FieldElement::ZERO,
        y: FieldElement::ZERO,
        infinity: true,
    };

    pub(crate) fn is_infinity(&self) -> bool {
        self.infinity || (self.x.is_zero() || self.y.is_zero())
    }

    pub(crate) fn from_be_bytes(x: &[u8; 32], y: &[u8; 32]) -> Result<Self, Secp256r1Err> {
        let x = FieldElement::from_be_bytes(x)?;
        let y = FieldElement::from_be_bytes(y)?;
        let infinity = x.is_zero() && y.is_zero();

        if infinity {
            return Ok(Affine::INFINITY);
        }

        let mut lhs = y;
        lhs.square_assign();

        let mut rhs = x;
        rhs.square_assign();
        rhs *= &x;

        let mut a = x;
        a *= &FieldElement::EQUATION_A;

        rhs += &a;
        rhs += &FieldElement::EQUATION_B;

        if rhs == lhs {
            Ok(Affine { x, y, infinity })
        } else {
            Err(Secp256r1Err::InvalidCoordinates)
        }
    }

    pub(crate) fn reject_identity(self) -> Result<Self, Secp256r1Err> {
        if self.is_infinity() {
            Err(Secp256r1Err::RecoveredInfinity)
        } else {
            Ok(self)
        }
    }

    pub(crate) fn to_jacobian(self) -> Jacobian {
        Jacobian {
            x: self.x,
            y: self.y,
            z: FieldElement::ONE,
        }
    }
}

impl Neg for Affine {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        self.y.negate_assign();
        self
    }
}

impl PartialEq for Affine {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.infinity == other.infinity
    }
}
