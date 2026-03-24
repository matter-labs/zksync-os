//! Oracle query processors for secp256k1 field operations (sqrt, inverse).
//!
//! Provides two implementations:
//! - [`FieldOpsQuery`]: Reads operands from simulated RISC-V memory.
//! - [`NativeFieldOpsQuery`]: Reads operands directly from native memory (for host execution).

use basic_system::system_functions::field_ops::{FieldHintOp, FieldOpsHint};
use basic_system::system_functions::field_ops::{FieldOpsHint64, FIELD_OPS_ADVISE_QUERY_ID};
use oracle_provider::OracleQueryProcessor;
use risc_v_simulator::abstractions::memory::MemorySource;
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::oracle::usize_serialization::UsizeSerializable;
use zk_ee::utils::Bytes32;
mod impls;

use crate::utils::evaluate::{read_memory_as_u64, read_struct};
use crate::{read_host_struct, read_u64_words};

pub struct FieldOpsQuery<M: MemorySource> {
    _marker: std::marker::PhantomData<M>,
}

impl<M: MemorySource> Default for FieldOpsQuery<M> {
    fn default() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M: MemorySource> OracleQueryProcessor<M> for FieldOpsQuery<M> {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![FIELD_OPS_ADVISE_QUERY_ID]
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        memory: &M,
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

        let arg = unsafe { read_struct::<FieldOpsHint, M>(memory, arg_ptr as u32) }.unwrap();

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

        match op {
            FieldHintOp::Secp256k1BaseFieldSqrt => {
                let t = impls::secp256k1_base_field_sqrt(n);
                DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
            }
            FieldHintOp::Secp256k1BaseFieldInverse => {
                let t = impls::secp256k1_base_field_inverse(n);
                DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
            }
            FieldHintOp::Secp256k1ScalarFieldInverse => {
                let t = impls::secp256k1_scalar_field_inverse(n);
                DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
            }
            _ => {
                panic!("Unknown field hint op {}", arg.op);
            }
        }
    }
}

pub struct NativeFieldOpsQuery<M: MemorySource> {
    _marker: std::marker::PhantomData<M>,
}

impl<M: MemorySource> Default for NativeFieldOpsQuery<M> {
    fn default() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M: MemorySource> OracleQueryProcessor<M> for NativeFieldOpsQuery<M> {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![FIELD_OPS_ADVISE_QUERY_ID]
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &M,
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
        assert_eq!(arg.src_len_u32_words, 8);
        let n: Vec<u64> = read_u64_words(arg.src_ptr, u64::from(arg.src_len_u32_words / 2));
        let n = Bytes32::from_array(
            n.into_iter()
                .flat_map(|el| el.to_le_bytes())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        );

        match op {
            FieldHintOp::Secp256k1BaseFieldSqrt => {
                let t = impls::secp256k1_base_field_sqrt(n);
                DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
            }
            FieldHintOp::Secp256k1BaseFieldInverse => {
                let t = impls::secp256k1_base_field_inverse(n);
                DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
            }
            FieldHintOp::Secp256k1ScalarFieldInverse => {
                let t = impls::secp256k1_scalar_field_inverse(n);
                DynUsizeIterator::from_constructor(t, UsizeSerializable::iter)
            }
            _ => {
                panic!("Unknown field hint op {}", arg.op);
            }
        }
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

        let output: Vec<usize> = NativeFieldOpsQuery::<DummyMemorySource>::default()
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
        let _ = NativeFieldOpsQuery::<DummyMemorySource>::default().process_buffered_query(
            FIELD_OPS_ADVISE_QUERY_ID,
            vec![0],
            &DummyMemorySource,
        );
    }
}
