use basic_system::system_functions::modexp::{
    ModExpAdviceParams, ModExpAdviceParams64, MODEXP_ADVICE_QUERY_ID,
};
use oracle_provider::OracleQueryProcessor;
use oracle_provider::RamPeek;
use zk_ee::oracle::memory_io::host::{QuerierMemory, ReadQueryInput, WriteQueryOutput};
use zk_ee::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_WIDE_DIV_REM_ADVICE_QUERY_ID};

use crate::utils::{
    evaluate::{read_memory_as_u64, read_struct},
    usize_slice_iterator::UsizeSliceIteratorOwned,
};
use crate::{read_host_struct, read_u64_words};

#[inline]
fn extract_single_ptr(query: Vec<usize>) -> usize {
    let mut it = query.into_iter();
    let ptr = it.next().expect("expected params pointer");
    assert!(it.next().is_none(), "expected exactly one pointer");
    ptr
}

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

/// Serves the U256 division advice (`u256_advice::U256DivRemAdviceQuery` and
/// `U256WideDivRemAdviceQuery`): the operands are read through the input word, from the guest memory or
/// from this process alike, and the answer is the quotient.
fn process_u256_advice_query(
    query_id: u32,
    input_word: usize,
    memory: &dyn QuerierMemory,
) -> Vec<u32> {
    let limbs = |value: &u256::U256| *value.as_limbs();
    let mut response = Vec::new();
    match query_id {
        U256_DIV_REM_ADVICE_QUERY_ID => {
            let (dividend, divisor) = <(u256::U256, u256::U256)>::read_input(memory, input_word)
                .expect("must read the div_rem operands");
            let mut quotient = limbs(&dividend);
            ruint::algorithms::div(&mut quotient, &mut limbs(&divisor));
            u256::U256::from_limbs(quotient).write_output(&mut response);
        }
        U256_WIDE_DIV_REM_ADVICE_QUERY_ID => {
            let (dividend_lo, dividend_hi, divisor) =
                <(u256::U256, u256::U256, u256::U256)>::read_input(memory, input_word)
                    .expect("must read the wide div_rem operands");
            let mut quotient = [0u64; 8];
            quotient[..4].copy_from_slice(&limbs(&dividend_lo));
            quotient[4..].copy_from_slice(&limbs(&dividend_hi));
            ruint::algorithms::div(&mut quotient, &mut limbs(&divisor));
            (
                u256::U256::from_limbs(quotient[..4].try_into().unwrap()),
                u256::U256::from_limbs(quotient[4..].try_into().unwrap()),
            )
                .write_output(&mut response);
        }
        _ => unreachable!("not a U256 advice query: 0x{query_id:08x}"),
    }
    response
}

fn process_modexp_riscv_query(
    query: Vec<usize>,
    memory: &dyn RamPeek,
) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
    let arg_ptr = extract_single_ptr(query);
    assert!(arg_ptr.is_multiple_of(4));
    const { assert!(core::mem::align_of::<ModExpAdviceParams>() <= 4) }
    const { assert!(core::mem::size_of::<ModExpAdviceParams>().is_multiple_of(4)) }
    let arg = unsafe { read_struct::<ModExpAdviceParams>(memory, arg_ptr as u32) }.unwrap();

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

fn process_modexp_native_query(
    query: Vec<usize>,
) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
    let arg_ptr = extract_single_ptr(query);
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

#[derive(Default)]
pub struct ArithmeticQuery;

impl OracleQueryProcessor for ArithmeticQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![MODEXP_ADVICE_QUERY_ID]
    }

    fn supported_memory_query_ids(&self) -> Vec<u32> {
        vec![
            U256_DIV_REM_ADVICE_QUERY_ID,
            U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
        ]
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
    ) -> Vec<u32> {
        process_u256_advice_query(query_id, input_word, memory)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        memory: &dyn RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        debug_assert!(self.supports_query_id(query_id));

        process_modexp_riscv_query(query, memory)
    }
}

/// Query processor to be used for prover input native run.
/// Works in a similar way as the ArithmeticQuery, but with
/// 64-bit pointers. For U256 div_rem and mulmod, the host
/// reads operands from process memory via raw pointer.
#[derive(Default)]
pub struct NativeArithmeticQuery;

impl OracleQueryProcessor for NativeArithmeticQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![MODEXP_ADVICE_QUERY_ID]
    }

    fn supported_memory_query_ids(&self) -> Vec<u32> {
        vec![
            U256_DIV_REM_ADVICE_QUERY_ID,
            U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
        ]
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
    ) -> Vec<u32> {
        process_u256_advice_query(query_id, input_word, memory)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &dyn RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        debug_assert!(self.supports_query_id(query_id));

        process_modexp_native_query(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_utils::TestMemorySource;
    use basic_system::system_functions::u256_advice::{
        U256DivRemAdviceQuery, U256WideDivRemAdviceQuery,
    };
    use oracle_provider::{DummyMemorySource, GuestMemory, ZkEENonDeterminismSource};
    use zk_ee::oracle::memory_io::OracleQuery;

    impl TestMemorySource {
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

    /// Helper: write a ModExpAdviceParams struct and data into TestMemorySource, then run the
    /// oracle query processor. Returns (quotient_u64_words, remainder_u64_words).
    fn run_division_query(dividend_u64: &[u64], modulus_u64: &[u64]) -> (Vec<u64>, Vec<u64>) {
        let a_digits = dividend_u64.len().div_ceil(4);
        let m_digits = modulus_u64.len().div_ceil(4);
        let a_u64_count = a_digits * 4;

        const PARAMS_ADDR: u32 = 0x100;
        const A_ADDR: u32 = 0x200;
        let m_addr: u32 = A_ADDR + (a_u64_count as u32) * 8;

        let mut memory = TestMemorySource::default();
        memory.insert_modexp_params(
            PARAMS_ADDR,
            ModExpAdviceParams {
                op: 0,
                a_ptr: A_ADDR,
                a_len: a_digits as u32,
                b_ptr: 0,
                b_len: 0,
                modulus_ptr: m_addr,
                modulus_len: m_digits as u32,
            },
        );
        memory.insert_u64_words(A_ADDR, dividend_u64);
        memory.insert_u64_words(m_addr, modulus_u64);

        let result: Vec<usize> = ArithmeticQuery
            .process_buffered_query(MODEXP_ADVICE_QUERY_ID, vec![PARAMS_ADDR as usize], &memory)
            .collect();

        assert!(!result.is_empty(), "Expected at least a header word");
        let header = result[0] as u64;
        let q_len_u32 = (header & 0xFFFF_FFFF) as usize;
        let r_len_u32 = (header >> 32) as usize;
        let q_len = q_len_u32 / 2;
        let r_len = r_len_u32 / 2;
        assert_eq!(result.len(), 1 + q_len + r_len);

        let quotient: Vec<u64> = result[1..1 + q_len].iter().map(|&x| x as u64).collect();
        let remainder: Vec<u64> = result[1 + q_len..].iter().map(|&x| x as u64).collect();
        (quotient, remainder)
    }

    #[test]
    fn riscv_arithmetic_query_basic_division() {
        // 10 / 3 = q=3, r=1
        let (q, r) = run_division_query(&[10, 0, 0, 0], &[3, 0, 0, 0]);
        assert_eq!(q, vec![3]);
        assert_eq!(r, vec![1]);
    }

    #[test]
    fn riscv_arithmetic_query_exact_division() {
        // 15 / 5 = q=3, r=0
        let (q, r) = run_division_query(&[15, 0, 0, 0], &[5, 0, 0, 0]);
        assert_eq!(q, vec![3]);
        assert!(r.is_empty(), "remainder should be zero (stripped)");
    }

    #[test]
    fn riscv_arithmetic_query_dividend_smaller_than_modulus() {
        // 2 / 7 = q=0, r=2
        let (q, r) = run_division_query(&[2, 0, 0, 0], &[7, 0, 0, 0]);
        assert!(q.is_empty(), "quotient should be zero (stripped)");
        assert_eq!(r, vec![2]);
    }

    #[test]
    fn riscv_arithmetic_query_dividend_fewer_digits_than_modulus() {
        // a=5 (1 DelegatedU256 digit), m=2^64+3 (2 DelegatedU256 digits)
        // 5 < 2^64+3, so q=0, r=5
        let (q, r) = run_division_query(&[5, 0, 0, 0], &[3, 0, 0, 0, 1, 0, 0, 0]);
        assert!(q.is_empty(), "quotient should be zero (stripped)");
        assert_eq!(r, vec![5]);
    }

    #[test]
    fn riscv_arithmetic_query_multi_digit_quotient() {
        // 2^128 / 3 = 0x55555555555555555555555555555555 remainder 1
        let (q, r) = run_division_query(&[0, 0, 1, 0], &[3, 0, 0, 0]);
        assert_eq!(q, vec![0x5555555555555555, 0x5555555555555555]);
        assert_eq!(r, vec![1]);
    }

    #[test]
    #[should_panic]
    fn riscv_arithmetic_query_division_by_zero() {
        let _ = run_division_query(&[10, 0, 0, 0], &[0, 0, 0, 0]);
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

        let output: Vec<usize> = NativeArithmeticQuery
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

    fn native_oracle() -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(NativeArithmeticQuery);
        oracle
    }

    #[test]
    fn u256_div_rem_via_native_query() {
        let dividend = u256::U256::from_limbs([10, 0, 0, 0]);
        let divisor = u256::U256::from_limbs([3, 0, 0, 0]);
        let quotient =
            U256DivRemAdviceQuery::get(&mut native_oracle(), (&dividend, &divisor)).unwrap();
        assert_eq!(*quotient.as_limbs(), [3, 0, 0, 0]);
    }

    #[test]
    fn u256_wide_div_rem_via_native_query() {
        // 35 / 6: q=5
        let dividend_lo = u256::U256::from_limbs([35, 0, 0, 0]);
        let dividend_hi = u256::U256::from_limbs([0, 0, 0, 0]);
        let divisor = u256::U256::from_limbs([6, 0, 0, 0]);
        let (q_lo, q_hi) = U256WideDivRemAdviceQuery::get(
            &mut native_oracle(),
            (&dividend_lo, &dividend_hi, &divisor),
        )
        .unwrap();
        assert_eq!(*q_lo.as_limbs(), [5, 0, 0, 0]);
        assert_eq!(*q_hi.as_limbs(), [0, 0, 0, 0]);
    }

    #[test]
    fn u256_wide_div_rem_large_dividend() {
        // 2^256 / (2^128 + 1): q = 2^128 - 1
        let dividend_lo = u256::U256::from_limbs([0, 0, 0, 0]);
        let dividend_hi = u256::U256::from_limbs([1, 0, 0, 0]);
        let divisor = u256::U256::from_limbs([1, 0, 1, 0]);
        let (q_lo, q_hi) = U256WideDivRemAdviceQuery::get(
            &mut native_oracle(),
            (&dividend_lo, &dividend_hi, &divisor),
        )
        .unwrap();
        assert_eq!(*q_lo.as_limbs(), [u64::MAX, u64::MAX, 0, 0]);
        assert_eq!(*q_hi.as_limbs(), [0, 0, 0, 0]);
    }

    #[test]
    fn u256_wide_div_rem_from_guest_memory() {
        // 2^256 / (2^128 + 1): q = 2^128 - 1, operands at 0x100, 0x200 and 0x300 of a 32-bit guest,
        // and the address array of the composite input at 0x400
        let mut memory = TestMemorySource::default();
        memory.insert_u64_words(0x100, &[0, 0, 0, 0]);
        memory.insert_u64_words(0x200, &[1, 0, 0, 0]);
        memory.insert_u64_words(0x300, &[1, 0, 1, 0]);
        for (i, address) in [0x100, 0x200, 0x300].into_iter().enumerate() {
            memory.insert_u32(0x400 + 4 * i as u32, address);
        }
        let response = ArithmeticQuery.process_memory_query(
            U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
            0x400,
            &GuestMemory(&memory),
        );
        // the two halves of the quotient, as the 16 words the guest reads: 2^128 - 1 fills the low 4
        let mut expected = vec![u32::MAX; 4];
        expected.extend([0; 12]);
        assert_eq!(response, expected);
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

        let riscv_output: Vec<usize> = ArithmeticQuery
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
        let native_output: Vec<usize> = NativeArithmeticQuery
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
    #[should_panic(expected = "expected params pointer")]
    fn arithmetic_query_panics_on_empty_query() {
        let memory = TestMemorySource::default();
        let _ = ArithmeticQuery.process_buffered_query(MODEXP_ADVICE_QUERY_ID, vec![], &memory);
    }

    #[test]
    #[should_panic(expected = "expected exactly one pointer")]
    fn arithmetic_query_panics_on_extra_args() {
        let memory = TestMemorySource::default();
        let _ = ArithmeticQuery.process_buffered_query(
            MODEXP_ADVICE_QUERY_ID,
            vec![0x100, 0x200],
            &memory,
        );
    }

    #[test]
    #[should_panic]
    fn arithmetic_query_panics_on_misaligned_pointer() {
        let memory = TestMemorySource::default();
        let _ =
            ArithmeticQuery.process_buffered_query(MODEXP_ADVICE_QUERY_ID, vec![0x101], &memory);
    }

    #[test]
    #[should_panic]
    fn native_arithmetic_query_rejects_null_query_pointer() {
        let _ = NativeArithmeticQuery.process_buffered_query(
            MODEXP_ADVICE_QUERY_ID,
            vec![0],
            &DummyMemorySource,
        );
    }
}
