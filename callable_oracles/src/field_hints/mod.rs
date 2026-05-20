//! Oracle query processors for secp256k1 field operations (sqrt, inverse).
//!
//! The processor decodes `FieldOpsInput` from WordLayout-encoded u32 words,
//! performs the computation, and returns the result as WordLayout-encoded u32 words.

use basic_system::system_functions::field_ops::{
    FieldHintOp, FieldOpsInput, FieldSqrtResponse, FIELD_OPS_ADVISE_QUERY_ID,
};
use oracle_provider::OracleQueryProcessor;
use oracle_provider::RamPeek;
use zk_ee::oracle::word_layout::WordLayout;
use zk_ee::system::errors::internal::InternalError;
mod impls;

/// Decode WordLayout-encoded u32 words back into a typed value.
fn decode_input<T: WordLayout>(input: &[u32]) -> T {
    let mut cursor = 0;
    T::read_words(&mut || {
        let w = input.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        w
    })
}

/// Unified field operations query processor. Handles both RISC-V and native queries
/// since inputs arrive WordLayout-encoded.
#[derive(Default)]
pub struct FieldOpsQuery;

impl OracleQueryProcessor for FieldOpsQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![FIELD_OPS_ADVISE_QUERY_ID]
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u32],
        _memory: &dyn RamPeek,
    ) -> Result<Vec<u32>, InternalError> {
        debug_assert!(self.supports_query_id(query_id));

        let field_input: FieldOpsInput = decode_input(input);

        let Some(op) = FieldHintOp::parse_u32(field_input.op) else {
            panic!("Unknown field hint op {}", field_input.op);
        };

        let n = field_input.src;

        let mut result = Vec::new();
        match op {
            FieldHintOp::Secp256k1BaseFieldSqrt => {
                let (candidate, is_qnr) = impls::secp256k1_base_field_sqrt(n);
                let response = FieldSqrtResponse {
                    sqrt_candidate: candidate,
                    is_quadratic_non_residue: is_qnr,
                };
                response.write_words(&mut |w| result.push(w));
            }
            FieldHintOp::Secp256k1BaseFieldInverse => {
                let inv = impls::secp256k1_base_field_inverse(n);
                inv.write_words(&mut |w| result.push(w));
            }
            FieldHintOp::Secp256k1ScalarFieldInverse => {
                let inv = impls::secp256k1_scalar_field_inverse(n);
                inv.write_words(&mut |w| result.push(w));
            }
            _ => {
                panic!("Unknown field hint op {}", field_input.op);
            }
        }
        Ok(result)
    }
}

pub use FieldOpsQuery as NativeFieldOpsQuery;

#[cfg(test)]
mod tests {
    use super::*;
    use oracle_provider::DummyMemorySource;

    #[test]
    fn field_ops_query_processes_inverse() {
        let mut input_bytes = [0u8; 32];
        input_bytes[31] = 1;
        let src = zk_ee::utils::Bytes32::from_array(input_bytes);
        let field_input = FieldOpsInput {
            op: FieldHintOp::Secp256k1BaseFieldInverse as u32,
            src,
        };
        let mut input_words = Vec::new();
        field_input.write_words(&mut |w| input_words.push(w));

        let output = NativeFieldOpsQuery
            .process(FIELD_OPS_ADVISE_QUERY_ID, &input_words, &DummyMemorySource)
            .unwrap();

        // Bytes32 = 8 u32 words
        assert_eq!(output.len(), 8);
        assert!(output.iter().any(|word| *word != 0));
    }

    #[test]
    fn field_ops_query_processes_sqrt() {
        let mut input_bytes = [0u8; 32];
        input_bytes[31] = 4; // 4 has a sqrt in the field
        let src = zk_ee::utils::Bytes32::from_array(input_bytes);
        let field_input = FieldOpsInput {
            op: FieldHintOp::Secp256k1BaseFieldSqrt as u32,
            src,
        };
        let mut input_words = Vec::new();
        field_input.write_words(&mut |w| input_words.push(w));

        let output = NativeFieldOpsQuery
            .process(FIELD_OPS_ADVISE_QUERY_ID, &input_words, &DummyMemorySource)
            .unwrap();

        // FieldSqrtResponse: Bytes32 (8 words) + bool (1 word) = 9 words
        assert_eq!(output.len(), 9);
    }
}
