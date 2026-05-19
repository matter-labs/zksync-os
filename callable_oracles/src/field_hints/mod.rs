//! Oracle query processors for secp256k1 field operations (sqrt, inverse).
//!
//! Provides two implementations:
//! - [`FieldOpsQuery`]: Reads operands from simulated RISC-V memory.
//! - [`NativeFieldOpsQuery`]: Reads operands directly from native memory (for host execution).

use basic_bootloader::bootloader::oracle_types::{FieldInverseResponse, FieldSqrtResponse};
use basic_system::system_functions::field_ops::{FieldHintOp, FieldOpsHint};
use basic_system::system_functions::field_ops::{FieldOpsHint64, FIELD_OPS_ADVISE_QUERY_ID};
use oracle_provider::OracleQueryProcessor;
use oracle_provider::RamPeek;
use zk_ee::internal_error;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::utils::Bytes32;
mod impls;

use crate::utils::evaluate::{read_memory_as_u64, read_struct};
use crate::{read_host_struct, read_u64_words};

fn compute_field_op(op: FieldHintOp, n: Bytes32) -> Result<Vec<u8>, InternalError> {
    match op {
        FieldHintOp::Secp256k1BaseFieldSqrt => {
            let (result, is_quadratic_non_residue) = impls::secp256k1_base_field_sqrt(n);
            let response = FieldSqrtResponse {
                result,
                is_valid: !is_quadratic_non_residue,
            };
            wincode::serialize(&response)
                .map_err(|_| internal_error!("encode field sqrt response failed"))
        }
        FieldHintOp::Secp256k1BaseFieldInverse => {
            let result = impls::secp256k1_base_field_inverse(n);
            let response = FieldInverseResponse { result };
            wincode::serialize(&response)
                .map_err(|_| internal_error!("encode field inverse response failed"))
        }
        FieldHintOp::Secp256k1ScalarFieldInverse => {
            let result = impls::secp256k1_scalar_field_inverse(n);
            let response = FieldInverseResponse { result };
            wincode::serialize(&response)
                .map_err(|_| internal_error!("encode scalar field inverse response failed"))
        }
        _ => {
            panic!("Unsupported field hint op");
        }
    }
}

#[derive(Default)]
pub struct FieldOpsQuery;

impl OracleQueryProcessor for FieldOpsQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![FIELD_OPS_ADVISE_QUERY_ID]
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u8],
        memory: &dyn RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        debug_assert!(self.supports_query_id(query_id));

        let arg_ptr: u32 = wincode::deserialize(input)
            .map_err(|_| internal_error!("decode field ops ptr failed"))?;

        assert!(arg_ptr.is_multiple_of(4));
        const { assert!(core::mem::align_of::<FieldOpsHint>() == 4) }
        const { assert!(core::mem::size_of::<FieldOpsHint>().is_multiple_of(4)) }

        let arg = unsafe { read_struct::<FieldOpsHint>(memory, arg_ptr) }.unwrap();

        let Some(op) = FieldHintOp::parse_u32(arg.op) else {
            panic!("Unknown field hint op {}", arg.op);
        };

        const { assert!(8 == core::mem::size_of::<usize>()) };
        assert!(arg.src_ptr > 0);
        assert_eq!(arg.src_len_u32_words, 8);
        let n = read_memory_as_u64(memory, arg.src_ptr as u32, arg.src_len_u32_words / 2).unwrap();

        let n = Bytes32::from_array(
            n.into_iter()
                .flat_map(|el| el.to_le_bytes())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        );

        compute_field_op(op, n)
    }
}

#[derive(Default)]
pub struct NativeFieldOpsQuery;

impl OracleQueryProcessor for NativeFieldOpsQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![FIELD_OPS_ADVISE_QUERY_ID]
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u8],
        _memory: &dyn RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        debug_assert!(self.supports_query_id(query_id));

        let arg_ptr: u64 = wincode::deserialize(input)
            .map_err(|_| internal_error!("decode field ops ptr failed"))?;
        let arg: FieldOpsHint64 = read_host_struct(arg_ptr);

        let op = FieldHintOp::parse_u32(arg.op)
            .unwrap_or_else(|| panic!("Unknown field hint op {}", arg.op));

        const { assert!(8 == core::mem::size_of::<usize>()) };
        assert!(arg.src_ptr > 0);
        assert_eq!(arg.src_len_u32_words, 8);
        let n: Vec<u64> = read_u64_words(arg.src_ptr, u64::from(arg.src_len_u32_words / 2));
        let n = Bytes32::from_array(
            n.into_iter()
                .flat_map(|el| el.to_le_bytes())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        );

        compute_field_op(op, n)
    }
}

#[cfg(test)]
mod native_query_tests {
    use super::*;
    use oracle_provider::DummyMemorySource;

    #[test]
    fn native_field_ops_query_processes_valid_query() {
        let mut input_data = [0u8; 32];
        input_data[31] = 1;
        let hint = FieldOpsHint64 {
            op: FieldHintOp::Secp256k1BaseFieldInverse as u32,
            src_ptr: input_data.as_ptr().addr() as u64,
            src_len_u32_words: 8,
        };

        let encoded_input =
            wincode::serialize(&((&hint as *const FieldOpsHint64).addr() as u64)).unwrap();
        let result_bytes = NativeFieldOpsQuery
            .process(
                FIELD_OPS_ADVISE_QUERY_ID,
                &encoded_input,
                &DummyMemorySource,
            )
            .unwrap();
        let response: FieldInverseResponse = wincode::deserialize(&result_bytes).unwrap();

        assert!(!response.result.is_zero());
    }

    #[test]
    #[should_panic]
    fn native_field_ops_query_rejects_null_query_pointer() {
        let encoded_input = wincode::serialize(&(0u64)).unwrap();
        let _ = NativeFieldOpsQuery.process(
            FIELD_OPS_ADVISE_QUERY_ID,
            &encoded_input,
            &DummyMemorySource,
        );
    }
}
