use super::Affine;
use crate::secp256r1::field::FieldElement;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Storage {
    pub(super) x: FieldElement,
    pub(super) y: FieldElement,
}

impl Storage {
    pub(crate) const DEFAULT: Self = Storage {
        x: FieldElement::ZERO,
        y: FieldElement::ZERO,
    };

    pub(crate) fn to_affine(self) -> Affine {
        Affine {
            x: self.x,
            y: self.y,
            infinity: false,
        }
    }
}
