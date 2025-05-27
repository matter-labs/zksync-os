use crate::secp256k1::field::FieldStorage;

use super::Affine;

#[derive(Debug, Clone, Copy)]

pub struct AffineStorage {
    pub(super) x: FieldStorage,
    pub(super) y: FieldStorage,
}

impl AffineStorage {
    pub(crate) const DEFAULT: Self = Self {
        x: FieldStorage::DEFAULT,
        y: FieldStorage::DEFAULT,
    };

    pub(crate) fn to_affine(self) -> Affine {
        Affine {
            x: self.x.to_field_elem(),
            y: self.y.to_field_elem(),
            infinity: false,
        }
    }
}
