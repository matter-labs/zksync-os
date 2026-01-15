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

#[cfg(test)]
mod tests {
    use super::*;
    use callable_oracles::field_hints::NativeFieldOpsQuery;
    use crypto::secp256k1::hooks::{DefaultSecp256k1Hooks, Secp256k1Hooks};
    use oracle_provider::{DummyMemorySource, ZkEENonDeterminismSource};
    use proptest::{prop_assert_eq, proptest};

    fn create_oracle_with_field_ops() -> ZkEENonDeterminismSource<DummyMemorySource> {
        let mut oracle = ZkEENonDeterminismSource::<DummyMemorySource>::default();
        oracle.add_external_processor(NativeFieldOpsQuery::<DummyMemorySource>::default());
        oracle
    }

    #[test]
    fn test_fe_sqrt_oracle_matches_default() {
        proptest!(|(bytes: [u8; 32])| {
            let Some(fe) = FieldElement::from_bytes(&bytes) else {
                return Ok(());
            };
            if fe.normalizes_to_zero() {
                return Ok(());
            }

            let mut fe_default = fe;
            let result_default = DefaultSecp256k1Hooks.fe_sqrt_and_assign(&mut fe_default);

            let mut oracle = create_oracle_with_field_ops();
            let mut fe_oracle = fe;
            let result_oracle = Secp256k1HooksWithOracle::new(&mut oracle)
                .fe_sqrt_and_assign(&mut fe_oracle);

            prop_assert_eq!(result_default, result_oracle, "sqrt existence should match");
            prop_assert_eq!(fe_default.to_bytes(), fe_oracle.to_bytes(), "sqrt values should match");
        });
    }

    #[test]
    fn test_fe_invert_oracle_matches_default() {
        proptest!(|(bytes: [u8; 32])| {
            let Some(fe) = FieldElement::from_bytes(&bytes) else {
                return Ok(());
            };
            if fe.normalizes_to_zero() {
                return Ok(());
            }

            let mut fe_default = fe;
            DefaultSecp256k1Hooks.fe_invert_and_assign(&mut fe_default);

            let mut oracle = create_oracle_with_field_ops();
            let mut fe_oracle = fe;
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe_oracle);

            prop_assert_eq!(fe_default.to_bytes(), fe_oracle.to_bytes(), "inverse values should match");
        });
    }

    #[test]
    fn test_scalar_invert_oracle_matches_default() {
        proptest!(|(bytes: [u8; 32])| {
            use crypto::k256::elliptic_curve::scalar::FromUintUnchecked;
            use crypto::k256::elliptic_curve::Curve;
            use crypto::k256::U256;

            let val = U256::from_be_slice(&bytes);
            if val >= crypto::k256::Secp256k1::ORDER || val == U256::ZERO {
                return Ok(());
            }

            let scalar = Scalar::from_k256_scalar(
                crypto::k256::Scalar::from_uint_unchecked(val)
            );

            let mut scalar_default = scalar;
            DefaultSecp256k1Hooks.scalar_invert_and_assign(&mut scalar_default);

            let mut oracle = create_oracle_with_field_ops();
            let mut scalar_oracle = scalar;
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar_oracle);

            prop_assert_eq!(scalar_default.to_repr(), scalar_oracle.to_repr(), "scalar inverse values should match");
        });
    }
}
