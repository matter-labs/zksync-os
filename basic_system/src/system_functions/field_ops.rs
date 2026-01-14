//! Query for field operations hints, such as square root and inverse in secp256k1 fields together with their use for implementing secp256k1 hooks.

use crypto::secp256k1::{FieldElement, Scalar};
use zk_ee::{
    oracle::{query_ids::ADVICE_SUBSPACE_MASK, IOOracle},
    utils::Bytes32,
};

pub const FIELD_OPS_ADVISE_QUERY_ID: u32 = ADVICE_SUBSPACE_MASK | 0x11;

#[repr(C)]
#[derive(Debug, Default)]
pub struct GenericFieldOpsHint<W> {
    pub op: u32,
    pub src_ptr: W,
    pub src_len_u32_words: u32,
}

pub type FieldOpsHint = GenericFieldOpsHint<u32>;
pub type FieldOpsHint64 = GenericFieldOpsHint<u64>;

#[repr(u32)]
#[non_exhaustive]
pub enum FieldHintOp {
    Secp256k1BaseFieldSqrt = 0,
    Secp256k1BaseFieldInverse,
    Secp256k1ScalarFieldInverse,
}

impl FieldHintOp {
    pub fn parse_u32(value: u32) -> Option<Self> {
        match value {
            a if a == (Self::Secp256k1BaseFieldSqrt as u32) => Some(Self::Secp256k1BaseFieldSqrt),
            a if a == (Self::Secp256k1BaseFieldInverse as u32) => {
                Some(Self::Secp256k1BaseFieldInverse)
            }
            a if a == (Self::Secp256k1ScalarFieldInverse as u32) => {
                Some(Self::Secp256k1ScalarFieldInverse)
            }
            _ => None,
        }
    }
}

/// Secp256k1 hooks implementation that uses an IOOracle for field operations.
pub struct Secp256k1HooksWithOracle<'a, O: IOOracle> {
    oracle: &'a mut O,
}

impl<'a, O: IOOracle> Secp256k1HooksWithOracle<'a, O> {
    pub fn new(oracle: &'a mut O) -> Self {
        Self { oracle }
    }
}

impl<'a, O: IOOracle> crypto::secp256k1::hooks::Secp256k1Hooks for Secp256k1HooksWithOracle<'a, O> {
    fn fe_sqrt_and_assign(&mut self, x: &mut FieldElement) -> bool {
        let input = Bytes32::from_array(x.to_bytes().into());
        // We use different advice params depending on architecture
        // They are mostly the same, main difference is the width of pointers
        #[cfg(target_arch = "riscv32")]
        let (sqrt_candidate, is_quadratic_non_residue): (Bytes32, bool) = {
            let hint_request = FieldOpsHint {
                op: FieldHintOp::Secp256k1BaseFieldSqrt as u32,
                src_ptr: input.as_u8_array_ref().as_ptr().addr() as u32,
                src_len_u32_words: 8,
            };
            self.oracle
                .query_serializable(
                    FIELD_OPS_ADVISE_QUERY_ID,
                    &((&hint_request as *const FieldOpsHint).addr() as u32),
                )
                .unwrap()
        };
        #[cfg(not(target_arch = "riscv32"))]
        let (sqrt_candidate, is_quadratic_non_residue): (Bytes32, bool) = {
            let hint_request = FieldOpsHint64 {
                op: FieldHintOp::Secp256k1BaseFieldSqrt as u32,
                src_ptr: input.as_u8_array_ref().as_ptr().addr() as u64,
                src_len_u32_words: 8,
            };
            self.oracle
                .query_serializable(
                    FIELD_OPS_ADVISE_QUERY_ID,
                    &((&hint_request as *const FieldOpsHint64).addr() as u64),
                )
                .unwrap()
        };

        // Answer must be a valid field element
        let fe = FieldElement::from_bytes(sqrt_candidate.as_u8_array_ref()).unwrap();

        // Verify the oracle's hint is correct.
        // The oracle computes candidate = x^((p+1)/4). For secp256k1's prime p ≡ 3 (mod 4):
        // - If x is a quadratic residue (has a sqrt): candidate² == x
        // - If x is a quadratic non-residue (no sqrt): candidate² == -x
        let mut squared = fe;
        squared.square_in_place();
        if is_quadratic_non_residue == false {
            squared.sub_in_place(&x);
            assert!(squared.normalizes_to_zero()); // candidate² - x == 0
        } else {
            squared.add_in_place(&x);
            assert!(squared.normalizes_to_zero()); // candidate² + x == 0  (i.e., candidate² == -x)
        }

        *x = fe;
        // Return true if square root exists (x is a quadratic residue)
        !is_quadratic_non_residue
    }

    fn fe_invert_and_assign(&mut self, x: &mut crypto::secp256k1::FieldElement) {
        use crate::system_functions::field_ops::FIELD_OPS_ADVISE_QUERY_ID;
        let input = Bytes32::from_array(x.to_bytes().into());
        // We use different advice params depending on architecture
        // They are mostly the same, main difference is the width of pointers
        #[cfg(target_arch = "riscv32")]
        let inv: Bytes32 = {
            let hint_request = FieldOpsHint {
                op: FieldHintOp::Secp256k1BaseFieldInverse as u32,
                src_ptr: input.as_u8_array_ref().as_ptr().addr() as u32,
                src_len_u32_words: 8,
            };
            self.oracle
                .query_serializable(
                    FIELD_OPS_ADVISE_QUERY_ID,
                    &((&hint_request as *const FieldOpsHint).addr() as u32),
                )
                .unwrap()
        };
        #[cfg(not(target_arch = "riscv32"))]
        let inv: Bytes32 = {
            let hint_request = FieldOpsHint64 {
                op: FieldHintOp::Secp256k1BaseFieldInverse as u32,
                src_ptr: input.as_u8_array_ref().as_ptr().addr() as u64,
                src_len_u32_words: 8,
            };
            self.oracle
                .query_serializable(
                    FIELD_OPS_ADVISE_QUERY_ID,
                    &((&hint_request as *const FieldOpsHint64).addr() as u64),
                )
                .unwrap()
        };

        // answer must be a field element
        let inv = FieldElement::from_bytes(inv.as_u8_array_ref()).unwrap();

        // we must check that hint was correct
        let mut t = *x;
        t *= inv;
        t.sub_in_place(&FieldElement::ONE);
        assert!(t.normalizes_to_zero());

        *x = inv;
    }

    fn scalar_invert_and_assign(&mut self, x: &mut crypto::secp256k1::Scalar) {
        use crate::system_functions::field_ops::FIELD_OPS_ADVISE_QUERY_ID;
        let input = Bytes32::from_array(x.to_repr().into());
        // We use different advice params depending on architecture
        // They are mostly the same, main difference is the width of pointers
        #[cfg(target_arch = "riscv32")]
        let inverse: Bytes32 = {
            let hint_request = FieldOpsHint {
                op: FieldHintOp::Secp256k1ScalarFieldInverse as u32,
                src_ptr: input.as_u8_array_ref().as_ptr().addr() as u32,
                src_len_u32_words: 8,
            };
            self.oracle
                .query_serializable(
                    FIELD_OPS_ADVISE_QUERY_ID,
                    &((&hint_request as *const FieldOpsHint).addr() as u32),
                )
                .unwrap()
        };
        #[cfg(not(target_arch = "riscv32"))]
        let inverse: Bytes32 = {
            let hint_request = FieldOpsHint64 {
                op: FieldHintOp::Secp256k1ScalarFieldInverse as u32,
                src_ptr: input.as_u8_array_ref().as_ptr().addr() as u64,
                src_len_u32_words: 8,
            };
            self.oracle
                .query_serializable(
                    FIELD_OPS_ADVISE_QUERY_ID,
                    &((&hint_request as *const FieldOpsHint64).addr() as u64),
                )
                .unwrap()
        };
        // answer is must be a field element
        use crypto::k256::elliptic_curve::scalar::FromUintUnchecked;
        use crypto::k256::elliptic_curve::Curve;
        use crypto::k256::U256;

        let inverse = U256::from_be_slice(inverse.as_u8_array_ref());
        assert!(inverse < crypto::k256::Secp256k1::ORDER);
        let inverse: Scalar =
            Scalar::from_k256_scalar(crypto::k256::Scalar::from_uint_unchecked(inverse));
        let mut t = *x;
        t *= inverse;
        t = t - Scalar::ONE;
        assert!(t.is_zero());

        *x = inverse;
    }
}
