use core::mem::MaybeUninit;

use ark_ff::BigInt;
use num_bigint::BigUint;
use oracle_provider::OracleQueryProcessor;
use risc_v_simulator::{abstractions::memory::{AccessType, MemorySource}, cycle::status_registers::TrapReason};
use ruint::aliases::U256;
use zk_ee::{kv_markers::UsizeDeserializable, system::{}, utils::Bytes32};

use crate::{utils::{evaluate::{read_memory_as_u8, write_memory_as_u8}, usize_slice_iterator::UsizeSliceIteratorOwned}, MemoryRegionDescriptionParams};

pub struct ArithmeticQuery<M: MemorySource> {
    pub marker: std::marker::PhantomData<M>,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct ArithmeticsParam {
    op: u32,
    a_ptr: u32,
    a_len: u32,
    b_ptr: u32,
    b_len: u32,
    c_ptr: u32,
    c_len: u32,
}

impl<M: MemorySource> OracleQueryProcessor<M> for ArithmeticQuery<M> {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![0x101]
    }

    fn process_buffered_query(
            &mut self,
            query_id: u32,
            query: Vec<usize>,
            memory: &M,
        ) -> Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>> {
        
        debug_assert!(self.supports_query_id(query_id));


        let mut it = query.into_iter();

        let arg_ptr = it.next().expect("A u32 should've been passed in.");

        assert!(arg_ptr % 4 == 0);
        const { assert!(core::mem::align_of::<ArithmeticsParam>() == 4) }
        const { assert!(core::mem::size_of::<ArithmeticsParam>() % 4 == 0) }

        let mut arg = MaybeUninit::<ArithmeticsParam>::uninit();

        let ptr = arg.as_mut_ptr();

        for i in (0 .. core::mem::size_of::<ArithmeticsParam>()).step_by(4) {
            let v = memory.get(arg_ptr as u64 + i as u64, AccessType::MemLoad, &mut TrapReason::NoTrap);
            println!("v{} {}", i, v);
            unsafe { ptr.cast::<u32>().add(i / 4).write(v) };
        }

        let arg = unsafe { arg.assume_init() };

        let n = read_memory_as_u8(
            memory,
            arg.a_ptr,
            arg.a_len * 32).unwrap();

        let d = read_memory_as_u8(
            memory,
            arg.b_ptr as u32,
            arg.b_len as u32 * 32).unwrap();

        let n = BigUint::from_bytes_le(&n);
        let d = BigUint::from_bytes_le(&d);

        let mut n = n.to_u64_digits();
        let mut d = d.to_u64_digits();
        let mut r = vec![((n.len() as u64 * 2) << 32) | d.len() as u64 * 2];

        const { assert!(8 == core::mem::size_of::<usize>()) };

        ruint::algorithms::div(&mut n, &mut d);

        r.extend_from_slice(&n);
        r.extend_from_slice(&d);
        let r = r.into_iter().map(|x| x as usize).collect::<Vec<_>>();
        let r = Vec::into_boxed_slice(r);

        let n = UsizeSliceIteratorOwned::new(r);

        println!("{:?}", n);

        Some(Box::new(n))
    }
}

pub fn wide_mod(
    n: U256,
    d: U256,
) -> (U256,U256) {
    let r = n.div_rem(d);
    r
}

#[repr(packed(8))]
pub struct ModExpCallInputs {
    n: U256,
    d: U256,
}

