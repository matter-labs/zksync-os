use oracle_provider::OracleQueryProcessor;
use risc_v_simulator::abstractions::memory::MemorySource;

use crate::utils::{
    evaluate::{read_memory_as_u64, read_struct},
    usize_slice_iterator::UsizeSliceIteratorOwned,
};

pub struct ArithmeticQuery<M: MemorySource> {
    pub marker: std::marker::PhantomData<M>,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct ArithmeticsParam {
    pub op: u32,
    pub a_ptr: u32,
    pub a_len: u32,
    pub b_ptr: u32,
    pub b_len: u32,
    pub modulus_ptr: u32,
    pub modulus_len: u32,
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

        assert!(
            it.next().is_none(),
            "A single RISC-V ptr should've been passed."
        );

        assert!(arg_ptr % 4 == 0);
        const { assert!(core::mem::align_of::<ArithmeticsParam>() == 4) }
        const { assert!(core::mem::size_of::<ArithmeticsParam>() % 4 == 0) }

        let arg = unsafe { read_struct::<ArithmeticsParam, _>(memory, arg_ptr as u32) }.unwrap();

        const { assert!(8 == core::mem::size_of::<usize>()) };
        assert!(arg.a_ptr > 0);
        let mut n = read_memory_as_u64(memory, arg.a_ptr, arg.a_len * 4).unwrap();
        assert_eq!(arg.b_ptr, 0);
        assert_eq!(arg.b_len, 0);
        assert!(arg.modulus_ptr > 0);
        assert!(arg.modulus_len > 0);
        let mut d = read_memory_as_u64(memory, arg.modulus_ptr, arg.modulus_len * 4).unwrap();

        ruint::algorithms::div(&mut n, &mut d);

        // Trim zeros
        fn strip_leading_zeroes(input: &[u64]) -> &[u64] {
            let mut digits = input.len();
            for el in input.iter().rev() {
                if *el == 0 {
                    digits -= 1;
                } else {
                    break;
                }
            }
            &input[..digits]
        }
        let quotient = strip_leading_zeroes(&n);
        let remainder = strip_leading_zeroes(&d);

        // account for usize being u64 here
        let q_len_in_u32_words = quotient.len() * 2;
        let r_len_in_u32_words = remainder.len() * 2;
        // account for LE, and we will ask quotient first, then remainder
        let header = [(q_len_in_u32_words as u64) | ((r_len_in_u32_words as u64) << 32)];

        let r = header
            .iter()
            .chain(quotient.iter())
            .chain(remainder.iter())
            .map(|x| *x as usize)
            .collect::<Vec<_>>();
        let r = Vec::into_boxed_slice(r);

        let n = UsizeSliceIteratorOwned::new(r);

        Some(Box::new(n))
    }
}
