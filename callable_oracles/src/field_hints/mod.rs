//! Oracle query processors for field operation hints (square roots and inverses of the
//! secp256k1, bn254 and bls12-381 fields).
//!
//! Provides two implementations:
//! - [`FieldOpsQuery`]: Reads operands from simulated RISC-V memory.
//! - [`NativeFieldOpsQuery`]: Reads operands directly from native memory (for host execution).

use basic_system::system_functions::field_ops::{FieldHintOp, FieldOpsHint};
use basic_system::system_functions::field_ops::{FieldOpsHint64, FIELD_OPS_ADVISE_QUERY_ID};
use oracle_provider::OracleQueryProcessor;
use oracle_provider::RamPeek;
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::oracle::usize_serialization::UsizeSerializable;
use zk_ee::utils::Bytes32;
mod impls;

use crate::utils::evaluate::{read_memory_as_u64, read_struct};
use crate::{read_host_struct, read_u64_words};

#[derive(Default)]
pub struct FieldOpsQuery;

impl OracleQueryProcessor for FieldOpsQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![FIELD_OPS_ADVISE_QUERY_ID]
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        memory: &dyn RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        debug_assert!(self.supports_query_id(query_id));

        let mut it = query.into_iter();

        let arg_ptr = it.next().expect("A u32 should've been passed in.");

        assert!(
            it.next().is_none(),
            "A single RISC-V ptr should've been passed."
        );

        assert!(arg_ptr.is_multiple_of(4));
        const { assert!(core::mem::align_of::<FieldOpsHint>() == 4) }
        const { assert!(core::mem::size_of::<FieldOpsHint>().is_multiple_of(4)) }

        let arg = unsafe { read_struct::<FieldOpsHint>(memory, arg_ptr as u32) }.unwrap();

        let Some(op) = FieldHintOp::parse_u32(arg.op) else {
            panic!("Unknown field hint op {}", arg.op);
        };

        const { assert!(8 == core::mem::size_of::<usize>()) };
        assert!(arg.src_ptr > 0);
        assert!(op.accepts_input_len_u32_words(arg.src_len_u32_words));
        let n = read_memory_as_u64(memory, arg.src_ptr, arg.src_len_u32_words / 2).unwrap();
        let bytes: Vec<u8> = n.into_iter().flat_map(|el| el.to_le_bytes()).collect();

        answer(op, &bytes)
    }
}

/// The answer to the hint `op` on the operand `bytes` (of `op.input_len_u32_words()` words)
fn answer(
    op: FieldHintOp,
    bytes: &[u8],
) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
    let bytes32 = || Bytes32::from_array(bytes.try_into().expect("a 32-byte operand"));
    match op {
        FieldHintOp::Secp256k1BaseFieldSqrt => {
            let t = impls::secp256k1_base_field_sqrt(bytes32());
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        FieldHintOp::Secp256k1BaseFieldInverse => {
            let t = impls::secp256k1_base_field_inverse(bytes32());
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        FieldHintOp::Secp256k1ScalarFieldInverse => {
            let t = impls::secp256k1_scalar_field_inverse(bytes32());
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        FieldHintOp::Bn254BaseFieldInverse => {
            let t: [Bytes32; 1] = impls::inverse::<crypto::bn254::Fq, 1>(bytes);
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        FieldHintOp::Bn254Fq12Inverse => {
            let t: [Bytes32; 12] = impls::inverse::<crypto::bn254::Fq12, 12>(bytes);
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        FieldHintOp::Bls12381BaseFieldSqrt => {
            let t: ([Bytes32; 2], bool) = impls::bls12_381_base_field_sqrt(bytes);
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        FieldHintOp::Bls12381BaseFieldInverse => {
            let t: [Bytes32; 2] = impls::inverse::<crypto::bls12_381::Fq, 2>(bytes);
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        FieldHintOp::Bls12381Fq12Inverse => {
            let t: [Bytes32; 24] = impls::inverse::<crypto::bls12_381::Fq12, 24>(bytes);
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        FieldHintOp::Bn254PairingResidueWitness => {
            let t: (bool, ([Bytes32; 12], ([Bytes32; 12], [Bytes32; 6]))) =
                impls::bn254_pairing_residue_witness(bytes, false);
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        FieldHintOp::Bls12381KzgResidueWitness => {
            let t: (bool, ([Bytes32; 24], [Bytes32; 12])) =
                impls::bls12_381_kzg_residue_witness(bytes, false);
            DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
        }
        _ => {
            panic!("Unknown field hint op {}", op as u32);
        }
    }
}

/// The operand of a request (`src_ptr`, `src_len_u32_words` of the request) in native memory
fn native_operand(op: FieldHintOp, src_ptr: u64, src_len_u32_words: u32) -> Vec<u8> {
    const { assert!(8 == core::mem::size_of::<usize>()) };
    assert!(src_ptr > 0);
    assert!(op.accepts_input_len_u32_words(src_len_u32_words));
    let n: Vec<u64> = read_u64_words(src_ptr, u64::from(src_len_u32_words / 2));
    n.into_iter().flat_map(|el| el.to_le_bytes()).collect()
}

/// The answer to a bn254 pairing request (its operand at `src_ptr`, `src_len_u32_words`
/// long, in native memory) that claims a non-identity, with the inverse the exact path needs,
/// whatever the product is: for tests of that path
pub fn bn254_pairing_not_identity_claim(src_ptr: u64, src_len_u32_words: u32) -> Vec<usize> {
    let operand = native_operand(
        FieldHintOp::Bn254PairingResidueWitness,
        src_ptr,
        src_len_u32_words,
    );
    let t = impls::bn254_pairing_residue_witness(&operand, true);
    t.iter().collect()
}

/// The answer to a KZG pairing request that claims a non-identity, as
/// `bn254_pairing_not_identity_claim`
pub fn bls12_381_kzg_not_identity_claim(src_ptr: u64, src_len_u32_words: u32) -> Vec<usize> {
    let operand = native_operand(
        FieldHintOp::Bls12381KzgResidueWitness,
        src_ptr,
        src_len_u32_words,
    );
    let t = impls::bls12_381_kzg_residue_witness(&operand, true);
    t.iter().collect()
}

#[derive(Default)]
pub struct NativeFieldOpsQuery;

impl OracleQueryProcessor for NativeFieldOpsQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![FIELD_OPS_ADVISE_QUERY_ID]
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &dyn RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        debug_assert!(self.supports_query_id(query_id));

        let mut it = query.into_iter();
        let arg_ptr = it.next().expect("A u64 should've been passed in.");
        assert!(it.next().is_none(), "A single ptr should've been passed.");
        let arg: FieldOpsHint64 = read_host_struct(arg_ptr as u64);

        let op = FieldHintOp::parse_u32(arg.op)
            .unwrap_or_else(|| panic!("Unknown field hint op {}", arg.op));

        const { assert!(8 == core::mem::size_of::<usize>()) };
        assert!(arg.src_ptr > 0);
        assert!(op.accepts_input_len_u32_words(arg.src_len_u32_words));
        let n: Vec<u64> = read_u64_words(arg.src_ptr, u64::from(arg.src_len_u32_words / 2));
        let bytes: Vec<u8> = n.into_iter().flat_map(|el| el.to_le_bytes()).collect();

        answer(op, &bytes)
    }
}

#[cfg(test)]
mod native_query_tests {
    use super::*;
    use oracle_provider::DummyMemorySource;

    #[test]
    fn native_field_ops_query_processes_valid_query() {
        let mut input = [0u8; 32];
        input[31] = 1;
        let hint = FieldOpsHint64 {
            op: FieldHintOp::Secp256k1BaseFieldInverse as u32,
            src_ptr: input.as_ptr().addr() as u64,
            src_len_u32_words: 8,
        };

        let output: Vec<usize> = NativeFieldOpsQuery
            .process_buffered_query(
                FIELD_OPS_ADVISE_QUERY_ID,
                vec![(&hint as *const FieldOpsHint64).addr()],
                &DummyMemorySource,
            )
            .collect();

        assert_eq!(output.len(), 4);
        assert!(output.iter().any(|word| *word != 0));
    }

    #[test]
    #[should_panic]
    fn native_field_ops_query_rejects_null_query_pointer() {
        let _ = NativeFieldOpsQuery.process_buffered_query(
            FIELD_OPS_ADVISE_QUERY_ID,
            vec![0],
            &DummyMemorySource,
        );
    }
}
