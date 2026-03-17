//! Query for field operations hints, such as square root and inverse in secp256k1 fields together with their use for implementing secp256k1 hooks.

use crypto::secp256k1::{FieldElement, Scalar};
use zk_ee::{
    oracle::{query_ids::ADVICE_SUBSPACE_MASK, usize_serialization::UsizeDeserializable, IOOracle},
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
        // Match default hook semantics: sqrt(0) exists and equals 0.
        if x.normalizes_to_zero() {
            return true;
        }

        let input = Bytes32::from_array(x.to_bytes().into());
        let (sqrt_candidate, is_quadratic_non_residue): (Bytes32, bool) =
            self.query_field_op(FieldHintOp::Secp256k1BaseFieldSqrt, &input);

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
        // Match default hook semantics: invert(0) == 0.
        if x.normalizes_to_zero() {
            return;
        }

        let input = Bytes32::from_array(x.to_bytes().into());
        let inv: Bytes32 = self.query_field_op(FieldHintOp::Secp256k1BaseFieldInverse, &input);

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
        // Match default hook semantics: invert(0) == 0.
        if x.is_zero() {
            return;
        }

        let input = Bytes32::from_array(x.to_repr().into());
        let inverse: Bytes32 =
            self.query_field_op(FieldHintOp::Secp256k1ScalarFieldInverse, &input);

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

impl<'a, O: IOOracle> Secp256k1HooksWithOracle<'a, O> {
    fn query_field_op<R: UsizeDeserializable>(&mut self, op: FieldHintOp, input: &Bytes32) -> R {
        // We use different advice params depending on architecture
        // They are mostly the same, main difference is the width of pointers
        #[cfg(target_arch = "riscv32")]
        let r: R = {
            let hint_request = FieldOpsHint {
                op: op as u32,
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
        let r: R = {
            let hint_request = FieldOpsHint64 {
                op: op as u32,
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
        r
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

    /// Tests that verify the validation logic catches lying oracles.
    /// These tests ensure that incorrect oracle responses are rejected.
    mod malicious_oracle_tests {
        use super::*;
        use oracle_provider::{MemorySource, OracleQueryProcessor};
        use proptest::prop_assert;

        /// Ways to corrupt oracle responses
        enum Corruption {
            /// Return all zeros
            ReturnZero,
            /// Flip the least significant bit of the result
            FlipLsb,
            /// Add 1 to the result (wrapping)
            AddOne,
            /// Return a fixed arbitrary value
            ReturnArbitrary([u8; 32]),
        }

        impl Corruption {
            fn apply(&self, data: &mut [u8]) {
                match self {
                    Corruption::ReturnZero => data.fill(0),
                    Corruption::FlipLsb => {
                        if !data.is_empty() {
                            data[data.len() - 1] ^= 1;
                        }
                    }
                    Corruption::AddOne => {
                        // Add 1 with carry propagation (big-endian)
                        let mut carry = 1u16;
                        for byte in data.iter_mut().rev() {
                            let sum = *byte as u16 + carry;
                            *byte = sum as u8;
                            carry = sum >> 8;
                        }
                    }
                    Corruption::ReturnArbitrary(val) => {
                        data.copy_from_slice(val);
                    }
                }
            }
        }

        /// A malicious oracle processor that wraps a correct one and corrupts its output
        struct LyingFieldOpsQuery<M: MemorySource> {
            inner: callable_oracles::field_hints::NativeFieldOpsQuery<M>,
            corruption: Corruption,
            /// If set, lie about sqrt existence (flip the boolean)
            lie_about_sqrt_existence: bool,
        }

        impl<M: MemorySource> LyingFieldOpsQuery<M> {
            fn new(corruption: Corruption) -> Self {
                Self {
                    inner: callable_oracles::field_hints::NativeFieldOpsQuery::default(),
                    corruption,
                    lie_about_sqrt_existence: false,
                }
            }

            fn with_sqrt_existence_lie(mut self) -> Self {
                self.lie_about_sqrt_existence = true;
                self
            }
        }

        impl<M: MemorySource> OracleQueryProcessor<M> for LyingFieldOpsQuery<M> {
            fn supported_query_ids(&self) -> Vec<u32> {
                self.inner.supported_query_ids()
            }

            fn process_buffered_query(
                &mut self,
                query_id: u32,
                query: Vec<usize>,
                memory: &M,
            ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
                // Get the correct response
                let correct_iter = self.inner.process_buffered_query(query_id, query, memory);
                let correct_response: Vec<usize> = correct_iter.collect();

                // Determine if this is a sqrt query (returns Bytes32 + bool) or inverse query (returns Bytes32)
                // sqrt response: 4 usize for Bytes32 + 1 usize for bool = 5 usize
                // inverse response: 4 usize for Bytes32 = 4 usize
                let is_sqrt_query = correct_response.len() == 5;

                let mut corrupted = correct_response.clone();

                if is_sqrt_query && self.lie_about_sqrt_existence {
                    // Flip the boolean (last element)
                    corrupted[4] ^= 1;
                } else {
                    // Corrupt the Bytes32 result (first 4 usize = 32 bytes)
                    let mut bytes = [0u8; 32];
                    for (i, &word) in corrupted[..4].iter().enumerate() {
                        bytes[i * 8..(i + 1) * 8].copy_from_slice(&word.to_le_bytes());
                    }
                    self.corruption.apply(&mut bytes);
                    for (i, chunk) in bytes.chunks(8).enumerate() {
                        corrupted[i] = usize::from_le_bytes(chunk.try_into().unwrap());
                    }
                }

                Box::new(corrupted.into_iter())
            }
        }

        fn create_lying_oracle(
            corruption: Corruption,
        ) -> ZkEENonDeterminismSource<DummyMemorySource> {
            let mut oracle = ZkEENonDeterminismSource::<DummyMemorySource>::default();
            oracle.add_external_processor(LyingFieldOpsQuery::<DummyMemorySource>::new(corruption));
            oracle
        }

        fn create_sqrt_existence_lying_oracle() -> ZkEENonDeterminismSource<DummyMemorySource> {
            let mut oracle = ZkEENonDeterminismSource::<DummyMemorySource>::default();
            oracle.add_external_processor(
                LyingFieldOpsQuery::<DummyMemorySource>::new(Corruption::ReturnZero)
                    .with_sqrt_existence_lie(),
            );
            oracle
        }

        // A known valid field element for testing (small value, definitely in field)
        fn test_field_element() -> FieldElement {
            let mut bytes = [0u8; 32];
            bytes[31] = 7; // Small non-zero value
            FieldElement::from_bytes(&bytes).unwrap()
        }

        fn test_scalar() -> Scalar {
            use crypto::k256::elliptic_curve::scalar::FromUintUnchecked;
            let mut bytes = [0u8; 32];
            bytes[31] = 7;
            Scalar::from_k256_scalar(crypto::k256::Scalar::from_uint_unchecked(
                crypto::k256::U256::from_be_slice(&bytes),
            ))
        }

        // ============ fe_invert tests ============

        #[test]
        #[should_panic]
        fn test_fe_invert_rejects_zero_answer() {
            let mut oracle = create_lying_oracle(Corruption::ReturnZero);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_invert_rejects_flipped_bit() {
            let mut oracle = create_lying_oracle(Corruption::FlipLsb);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_invert_rejects_off_by_one() {
            let mut oracle = create_lying_oracle(Corruption::AddOne);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_invert_rejects_arbitrary_value() {
            let arbitrary = [0x42u8; 32];
            let mut oracle = create_lying_oracle(Corruption::ReturnArbitrary(arbitrary));
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe);
        }

        // ============ fe_sqrt tests ============

        #[test]
        #[should_panic]
        fn test_fe_sqrt_rejects_wrong_sqrt_value() {
            let mut oracle = create_lying_oracle(Corruption::FlipLsb);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_sqrt_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_sqrt_rejects_zero_answer() {
            let mut oracle = create_lying_oracle(Corruption::ReturnZero);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_sqrt_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_sqrt_rejects_lie_about_existence() {
            // This test uses an oracle that returns the correct sqrt value but lies
            // about whether a sqrt exists (flips the boolean)
            let mut oracle = create_sqrt_existence_lying_oracle();
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_sqrt_and_assign(&mut fe);
        }

        // ============ scalar_invert tests ============

        #[test]
        #[should_panic]
        fn test_scalar_invert_rejects_zero_answer() {
            let mut oracle = create_lying_oracle(Corruption::ReturnZero);
            let mut scalar = test_scalar();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar);
        }

        #[test]
        #[should_panic]
        fn test_scalar_invert_rejects_flipped_bit() {
            let mut oracle = create_lying_oracle(Corruption::FlipLsb);
            let mut scalar = test_scalar();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar);
        }

        #[test]
        #[should_panic]
        fn test_scalar_invert_rejects_off_by_one() {
            let mut oracle = create_lying_oracle(Corruption::AddOne);
            let mut scalar = test_scalar();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar);
        }

        #[test]
        #[should_panic]
        fn test_scalar_invert_rejects_arbitrary_value() {
            let arbitrary = [0x42u8; 32];
            let mut oracle = create_lying_oracle(Corruption::ReturnArbitrary(arbitrary));
            let mut scalar = test_scalar();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar);
        }

        // ============ Proptest: random corruptions should be rejected ============

        #[test]
        fn test_fe_invert_rejects_random_corruptions() {
            proptest!(|(bytes: [u8; 32], corruption_bytes: [u8; 32])| {
                let Some(fe) = FieldElement::from_bytes(&bytes) else {
                    return Ok(());
                };
                if fe.normalizes_to_zero() {
                    return Ok(());
                }

                // Get the correct inverse first
                let mut correct_fe = fe;
                let mut correct_oracle = create_oracle_with_field_ops();
                Secp256k1HooksWithOracle::new(&mut correct_oracle).fe_invert_and_assign(&mut correct_fe);
                let correct_inverse = correct_fe.to_bytes();

                // Skip if random corruption happens to equal the correct answer
                if corruption_bytes == *correct_inverse {
                    return Ok(());
                }

                // Now try with the corrupted oracle
                let mut lying_oracle = create_lying_oracle(Corruption::ReturnArbitrary(corruption_bytes));
                let mut test_fe = fe;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Secp256k1HooksWithOracle::new(&mut lying_oracle).fe_invert_and_assign(&mut test_fe);
                }));

                // The validation should have caught the lie (panicked)
                prop_assert!(result.is_err(), "Oracle lie was not detected for input {:?}", bytes);
            });
        }

        #[test]
        fn test_scalar_invert_rejects_random_corruptions() {
            proptest!(|(bytes: [u8; 32], corruption_bytes: [u8; 32])| {
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

                // Get the correct inverse first
                let mut correct_scalar = scalar;
                let mut correct_oracle = create_oracle_with_field_ops();
                Secp256k1HooksWithOracle::new(&mut correct_oracle).scalar_invert_and_assign(&mut correct_scalar);
                let correct_inverse = correct_scalar.to_repr();

                // Skip if random corruption happens to equal the correct answer
                if corruption_bytes == *correct_inverse {
                    return Ok(());
                }

                // Also skip if corruption_bytes >= ORDER (would fail earlier validation)
                let corruption_val = U256::from_be_slice(&corruption_bytes);
                if corruption_val >= crypto::k256::Secp256k1::ORDER {
                    return Ok(());
                }

                // Now try with the corrupted oracle
                let mut lying_oracle = create_lying_oracle(Corruption::ReturnArbitrary(corruption_bytes));
                let mut test_scalar = scalar;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Secp256k1HooksWithOracle::new(&mut lying_oracle).scalar_invert_and_assign(&mut test_scalar);
                }));

                // The validation should have caught the lie (panicked)
                prop_assert!(result.is_err(), "Oracle lie was not detected for input {:?}", bytes);
            });
        }

        #[test]
        fn test_fe_sqrt_rejects_random_corruptions() {
            proptest!(|(bytes: [u8; 32], corruption_bytes: [u8; 32], flip_bool: bool)| {
                let Some(fe) = FieldElement::from_bytes(&bytes) else {
                    return Ok(());
                };
                if fe.normalizes_to_zero() {
                    return Ok(());
                }

                // Get the correct result first
                let mut correct_fe = fe;
                let mut correct_oracle = create_oracle_with_field_ops();
                let correct_exists = Secp256k1HooksWithOracle::new(&mut correct_oracle)
                    .fe_sqrt_and_assign(&mut correct_fe);
                let correct_sqrt = correct_fe.to_bytes();

                // Test 1: Corrupt the sqrt candidate value
                // Skip if random corruption happens to equal the correct answer
                if corruption_bytes != *correct_sqrt {
                    let mut lying_oracle = create_lying_oracle(Corruption::ReturnArbitrary(corruption_bytes));
                    let mut test_fe = fe;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        Secp256k1HooksWithOracle::new(&mut lying_oracle).fe_sqrt_and_assign(&mut test_fe);
                    }));

                    // The validation should have caught the lie (panicked)
                    prop_assert!(result.is_err(),
                        "Oracle lie about sqrt candidate was not detected for input {:?}", bytes);
                }

                // Test 2: Lie about sqrt existence (flip the boolean)
                // Only test if flip_bool is true to reduce test cases
                if flip_bool {
                    let mut lying_oracle = create_sqrt_existence_lying_oracle();
                    let mut test_fe = fe;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        Secp256k1HooksWithOracle::new(&mut lying_oracle).fe_sqrt_and_assign(&mut test_fe);
                    }));

                    // The validation should have caught the lie about existence (panicked)
                    prop_assert!(result.is_err(),
                        "Oracle lie about sqrt existence was not detected for input {:?} (is_qr={})",
                        bytes, correct_exists);
                }
            });
        }
    }

    mod zero_input_regression_tests {
        use super::*;
        use oracle_provider::{
            DummyMemorySource, MemorySource, OracleQueryProcessor, ZkEENonDeterminismSource,
        };

        /// A processor that should never be reached in these tests.
        /// Zero-input hook behavior is expected to short-circuit before oracle queries.
        struct PanickingFieldOpsQuery;

        impl<M: MemorySource> OracleQueryProcessor<M> for PanickingFieldOpsQuery {
            fn supported_query_ids(&self) -> Vec<u32> {
                vec![FIELD_OPS_ADVISE_QUERY_ID]
            }

            fn process_buffered_query(
                &mut self,
                query_id: u32,
                _query: Vec<usize>,
                _memory: &M,
            ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
                panic!("field ops oracle should not be queried for zero input, query_id=0x{query_id:08x}");
            }
        }

        fn create_panicking_field_ops_oracle() -> ZkEENonDeterminismSource<DummyMemorySource> {
            let mut oracle = ZkEENonDeterminismSource::<DummyMemorySource>::default();
            oracle.add_external_processor(PanickingFieldOpsQuery);
            oracle
        }

        fn zero_field_element() -> FieldElement {
            FieldElement::from_bytes(&[0u8; 32]).expect("zero is a valid field element")
        }

        #[test]
        fn test_fe_invert_zero_does_not_query_oracle() {
            let mut fe_default = zero_field_element();
            DefaultSecp256k1Hooks.fe_invert_and_assign(&mut fe_default);
            assert!(fe_default.normalizes_to_zero());

            let mut fe_oracle = zero_field_element();
            let mut oracle = create_panicking_field_ops_oracle();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe_oracle);
            assert!(fe_oracle.normalizes_to_zero());
        }

        #[test]
        fn test_scalar_invert_zero_does_not_query_oracle() {
            let mut scalar_default = Scalar::ZERO;
            DefaultSecp256k1Hooks.scalar_invert_and_assign(&mut scalar_default);
            assert!(scalar_default.is_zero());

            let mut scalar_oracle = Scalar::ZERO;
            let mut oracle = create_panicking_field_ops_oracle();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar_oracle);
            assert!(scalar_oracle.is_zero());
        }

        #[test]
        fn test_fe_sqrt_zero_does_not_query_oracle_and_matches_default() {
            let mut fe_default = zero_field_element();
            let exists_default = DefaultSecp256k1Hooks.fe_sqrt_and_assign(&mut fe_default);
            assert!(exists_default);
            assert!(fe_default.normalizes_to_zero());

            let mut fe_oracle = zero_field_element();
            let mut oracle = create_panicking_field_ops_oracle();
            let exists_oracle =
                Secp256k1HooksWithOracle::new(&mut oracle).fe_sqrt_and_assign(&mut fe_oracle);
            assert!(exists_oracle);
            assert!(fe_oracle.normalizes_to_zero());
        }
    }
}
