use basic_system::system_functions::modexp::{
    ModExpAdviceParams, ModExpAdviceParams64, MODEXP_ADVICE_QUERY_ID,
};
use oracle_provider::OracleQueryProcessor;
use risc_v_simulator::abstractions::memory::MemorySource;

use crate::read_u64_words;
use crate::utils::{
    evaluate::{read_memory_as_u64, read_struct},
    usize_slice_iterator::UsizeSliceIteratorOwned,
};

struct ArithmeticQueryOutput {
    quotient: Vec<u64>,
    remainder: Vec<u64>,
}

impl ArithmeticQueryOutput {
    fn into_usize_iterator(
        self,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
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
        let quotient = strip_leading_zeroes(&self.quotient);
        let remainder = strip_leading_zeroes(&self.remainder);

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

        Box::new(n)
    }
}

pub struct ArithmeticQuery<M: MemorySource> {
    _marker: std::marker::PhantomData<M>,
}

impl<M: MemorySource> Default for ArithmeticQuery<M> {
    fn default() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M: MemorySource> OracleQueryProcessor<M> for ArithmeticQuery<M> {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![MODEXP_ADVICE_QUERY_ID]
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
        const { assert!(core::mem::align_of::<ModExpAdviceParams>() == 4) }
        const { assert!(core::mem::size_of::<ModExpAdviceParams>().is_multiple_of(4)) }

        let arg = unsafe { read_struct::<ModExpAdviceParams, _>(memory, arg_ptr as u32) }.unwrap();

        const { assert!(8 == core::mem::size_of::<usize>()) };
        assert!(arg.a_ptr > 0);
        assert!(arg.a_len > 0);
        let mut n = read_memory_as_u64(memory, arg.a_ptr, arg.a_len * 4).unwrap();
        assert_eq!(arg.b_ptr, 0);
        assert_eq!(arg.b_len, 0);
        assert!(arg.modulus_ptr > 0);
        assert!(arg.modulus_len > 0);
        let mut d = read_memory_as_u64(memory, arg.modulus_ptr, arg.modulus_len * 4).unwrap();

        ruint::algorithms::div(&mut n, &mut d);

        ArithmeticQueryOutput {
            quotient: n,
            remainder: d,
        }
        .into_usize_iterator()
    }
}

/// Query processor to be used for prover input native run
/// Works in a similar way as the ArithmeticQuery, but with
/// 64 bit pointers. Importantly, the query response is the
/// same.
///
/// This processor explicitly reads the process memory
/// using a raw pointer to get the input.
pub struct NativeArithmeticQuery<M: MemorySource> {
    _marker: std::marker::PhantomData<M>,
}

impl<M: MemorySource> Default for NativeArithmeticQuery<M> {
    fn default() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M: MemorySource> OracleQueryProcessor<M> for NativeArithmeticQuery<M> {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![MODEXP_ADVICE_QUERY_ID]
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

        let arg = unsafe {
            let p = arg_ptr as *const ModExpAdviceParams64;
            core::ptr::read_unaligned(p)
        };

        assert!(arg.a_ptr > 0);
        assert!(arg.a_len > 0);

        assert_eq!(arg.b_ptr, 0);
        assert_eq!(arg.b_len, 0);

        assert!(arg.modulus_ptr > 0);
        assert!(arg.modulus_len > 0);

        let mut n: Vec<u64> = unsafe { read_u64_words(arg.a_ptr, arg.a_len * 4) };
        let mut d: Vec<u64> = unsafe { read_u64_words(arg.modulus_ptr, arg.modulus_len * 4) };

        ruint::algorithms::div(&mut n, &mut d);

        ArithmeticQueryOutput {
            quotient: n,
            remainder: d,
        }
        .into_usize_iterator()
    }
}
