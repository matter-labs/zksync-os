use basic_system::system_functions::modexp::{
    ModExpAdviceParams, ModExpAdviceParams64, MODEXP_ADVICE_QUERY_ID,
};
use oracle_provider::OracleQueryProcessor;
use risc_v_simulator::abstractions::memory::MemorySource;

use crate::utils::{
    evaluate::{read_memory_as_u64, read_struct},
    usize_slice_iterator::UsizeSliceIteratorOwned,
};
use crate::{read_host_struct, read_u64_words};

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
        let arg: ModExpAdviceParams64 = read_host_struct(arg_ptr as u64);

        assert!(arg.a_ptr > 0);
        assert!(arg.a_len > 0);
        assert_eq!(arg.b_ptr, 0);
        assert_eq!(arg.b_len, 0);
        assert!(arg.modulus_ptr > 0);
        assert!(arg.modulus_len > 0);

        let a_len_u64_words = arg.a_len.checked_mul(4).expect("a_len overflow");
        let modulus_len_u64_words = arg
            .modulus_len
            .checked_mul(4)
            .expect("modulus_len overflow");

        let mut n: Vec<u64> = read_u64_words(arg.a_ptr, a_len_u64_words);
        let mut d: Vec<u64> = read_u64_words(arg.modulus_ptr, modulus_len_u64_words);

        ruint::algorithms::div(&mut n, &mut d);

        ArithmeticQueryOutput {
            quotient: n,
            remainder: d,
        }
        .into_usize_iterator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use oracle_provider::DummyMemorySource;
    use risc_v_simulator::abstractions::memory::{AccessType, MemorySource};
    use risc_v_simulator::cycle::status_registers::TrapReason;

    #[derive(Default)]
    struct TestMemorySource {
        words: BTreeMap<u64, u32>,
    }

    impl TestMemorySource {
        fn insert_u32(&mut self, address: u32, value: u32) {
            assert!(address.is_multiple_of(4));
            self.words.insert(address as u64, value);
        }

        fn insert_u64_words(&mut self, address: u32, values: &[u64]) {
            for (idx, value) in values.iter().copied().enumerate() {
                let word_address = address + (idx as u32) * 8;
                self.insert_u32(word_address, value as u32);
                self.insert_u32(word_address + 4, (value >> 32) as u32);
            }
        }

        fn insert_modexp_params(&mut self, address: u32, params: ModExpAdviceParams) {
            for (idx, value) in [
                params.op,
                params.a_ptr,
                params.a_len,
                params.b_ptr,
                params.b_len,
                params.modulus_ptr,
                params.modulus_len,
            ]
            .into_iter()
            .enumerate()
            {
                self.insert_u32(address + (idx as u32) * 4, value);
            }
        }
    }

    impl MemorySource for TestMemorySource {
        fn get(&self, phys_address: u64, _access_type: AccessType, trap: &mut TrapReason) -> u32 {
            if let Some(value) = self.words.get(&phys_address).copied() {
                *trap = TrapReason::NoTrap;
                value
            } else {
                *trap = TrapReason::LoadAccessFault;
                0
            }
        }

        fn set(
            &mut self,
            phys_address: u64,
            value: u32,
            _access_type: AccessType,
            trap: &mut TrapReason,
        ) {
            self.words.insert(phys_address, value);
            *trap = TrapReason::NoTrap;
        }
    }

    fn patterned_u64_words(len: usize, seed: u64) -> Vec<u64> {
        let mut state = seed;
        let mut words = Vec::with_capacity(len);
        for idx in 0..len {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            let mut word = state
                .wrapping_mul(0x2545_F491_4F6C_DD1D)
                .wrapping_add(idx as u64);
            if word == 0 {
                word = seed.wrapping_add(idx as u64 + 1);
            }
            words.push(word);
        }

        words[0] |= 1;
        *words.last_mut().expect("large input must be non-empty") |= 1 << 63;

        words
    }

    #[test]
    fn native_arithmetic_query_processes_valid_query() {
        let mut dividend = vec![10u64, 0, 0, 0];
        let mut modulus = vec![3u64, 0, 0, 0];
        let arg = ModExpAdviceParams64 {
            op: 0,
            a_ptr: dividend.as_mut_ptr().addr() as u64,
            a_len: 1,
            b_ptr: 0,
            b_len: 0,
            modulus_ptr: modulus.as_mut_ptr().addr() as u64,
            modulus_len: 1,
        };

        let output: Vec<usize> = NativeArithmeticQuery::<DummyMemorySource>::default()
            .process_buffered_query(
                MODEXP_ADVICE_QUERY_ID,
                vec![(&arg as *const ModExpAdviceParams64).addr()],
                &DummyMemorySource,
            )
            .collect();

        assert_eq!(output.len(), 3);
        let packed_lens = output[0] as u64;
        assert_eq!(packed_lens as u32, 2);
        assert_eq!((packed_lens >> 32) as u32, 2);
        assert_eq!(output[1], 3);
        assert_eq!(output[2], 1);
    }

    #[test]
    fn native_and_riscv_arithmetic_queries_match_for_large_modexp_inputs() {
        const DIVIDEND_DIGITS: usize = 12;
        const MODULUS_DIGITS: usize = 8;
        const GUEST_ARG_ADDR: u32 = 0x1000;
        const GUEST_DIVIDEND_ADDR: u32 = 0x2000;
        const GUEST_MODULUS_ADDR: u32 = 0x4000;

        let mut dividend = patterned_u64_words(DIVIDEND_DIGITS * 4, 0x0123_4567_89AB_CDEF);
        let mut modulus = patterned_u64_words(MODULUS_DIGITS * 4, 0x0FED_CBA9_8765_4321);

        let mut memory = TestMemorySource::default();
        memory.insert_modexp_params(
            GUEST_ARG_ADDR,
            ModExpAdviceParams {
                op: 0,
                a_ptr: GUEST_DIVIDEND_ADDR,
                a_len: DIVIDEND_DIGITS as u32,
                b_ptr: 0,
                b_len: 0,
                modulus_ptr: GUEST_MODULUS_ADDR,
                modulus_len: MODULUS_DIGITS as u32,
            },
        );
        memory.insert_u64_words(GUEST_DIVIDEND_ADDR, &dividend);
        memory.insert_u64_words(GUEST_MODULUS_ADDR, &modulus);

        let riscv_output: Vec<usize> = ArithmeticQuery::<TestMemorySource>::default()
            .process_buffered_query(
                MODEXP_ADVICE_QUERY_ID,
                vec![GUEST_ARG_ADDR as usize],
                &memory,
            )
            .collect();

        let host_arg = ModExpAdviceParams64 {
            op: 0,
            a_ptr: dividend.as_mut_ptr().addr() as u64,
            a_len: DIVIDEND_DIGITS as u64,
            b_ptr: 0,
            b_len: 0,
            modulus_ptr: modulus.as_mut_ptr().addr() as u64,
            modulus_len: MODULUS_DIGITS as u64,
        };
        let native_output: Vec<usize> = NativeArithmeticQuery::<DummyMemorySource>::default()
            .process_buffered_query(
                MODEXP_ADVICE_QUERY_ID,
                vec![(&host_arg as *const ModExpAdviceParams64).addr()],
                &DummyMemorySource,
            )
            .collect();

        assert_eq!(native_output, riscv_output);

        let packed_lens = native_output[0] as u64;
        let q_len = packed_lens as u32;
        let r_len = (packed_lens >> 32) as u32;
        assert!(q_len.is_multiple_of(2));
        assert!(r_len.is_multiple_of(2));
        assert!(q_len > 2, "quotient should span multiple u64 limbs");
        assert!(r_len > 2, "remainder should span multiple u64 limbs");
    }

    #[test]
    #[should_panic]
    fn native_arithmetic_query_rejects_null_query_pointer() {
        let _ = NativeArithmeticQuery::<DummyMemorySource>::default().process_buffered_query(
            MODEXP_ADVICE_QUERY_ID,
            vec![0],
            &DummyMemorySource,
        );
    }
}
