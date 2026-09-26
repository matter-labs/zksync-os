use basic_system::system_functions::modexp::{read_modexp_advice_params, MODEXP_ADVICE_QUERY_ID};
use oracle_provider::{respond_to_every_target, OracleQueryProcessor, RunMode};
use zk_ee::oracle::memory_io::host::{
    read_querier_u32_words, QuerierMemory, ReadQueryInput, WriteQueryOutput,
};
use zk_ee::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_WIDE_DIV_REM_ADVICE_QUERY_ID};

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

/// Serves the modexp division advice (`modexp::advice::bigint::OracleAdvisor`): the request, a
/// `ModExpAdviceParamsGeneric` of querier words, and the operands it points to (256-bit digits) are
/// read through the input word, from the guest memory or from this process alike. The answer is the
/// lengths of the quotient and of the remainder in `u32` words, then their words.
fn process_modexp_query(input_word: usize, memory: &dyn QuerierMemory) -> Vec<u32> {
    let params =
        read_modexp_advice_params(memory, input_word).expect("must read the modexp advice request");
    assert!(params.a_ptr > 0);
    assert!(params.a_len > 0);
    assert_eq!(params.b_ptr, 0);
    assert_eq!(params.b_len, 0);
    assert!(params.modulus_ptr > 0);
    assert!(params.modulus_len > 0);

    // a 256-bit digit is 8 words
    let read_digits = |address: usize, digits: usize| -> Vec<u64> {
        let num_words = digits.checked_mul(8).expect("operand length overflow");
        read_querier_u32_words(memory, address, num_words)
            .expect("must read the modexp operand")
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[low, high]| u64::from(*low) | (u64::from(*high) << 32))
            .collect()
    };
    let mut n = read_digits(params.a_ptr, params.a_len);
    let mut d = read_digits(params.modulus_ptr, params.modulus_len);

    ruint::algorithms::div(&mut n, &mut d);

    // without the leading zero limbs
    fn significant(limbs: &[u64]) -> &[u64] {
        let zeroes = limbs.iter().rev().take_while(|limb| **limb == 0).count();
        &limbs[..limbs.len() - zeroes]
    }
    let (quotient, remainder) = (significant(&n), significant(&d));
    let mut response = Vec::with_capacity(2 + 2 * (quotient.len() + remainder.len()));
    for limbs in [quotient, remainder] {
        response.push(u32::try_from(2 * limbs.len()).expect("the modexp advice is too long"));
    }
    for limb in quotient.iter().chain(remainder) {
        response.push(*limb as u32);
        response.push((*limb >> 32) as u32);
    }
    response
}

fn process_arithmetic_query(
    query_id: u32,
    input_word: usize,
    memory: &dyn QuerierMemory,
) -> Vec<u32> {
    match query_id {
        MODEXP_ADVICE_QUERY_ID => process_modexp_query(input_word, memory),
        _ => process_u256_advice_query(query_id, input_word, memory),
    }
}

const ARITHMETIC_QUERY_IDS: [u32; 3] = [
    MODEXP_ADVICE_QUERY_ID,
    U256_DIV_REM_ADVICE_QUERY_ID,
    U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
];

/// Serves the arithmetic advice of a querier in the simulated RISC-V machine.
#[derive(Default)]
pub struct ArithmeticQuery;

impl OracleQueryProcessor for ArithmeticQuery {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        ARITHMETIC_QUERY_IDS.to_vec()
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
        mode: RunMode,
        native_run_responses: &mut Vec<u32>,
        guest_run_responses: &mut Vec<u32>,
    ) {
        respond_to_every_target(
            mode,
            process_arithmetic_query(query_id, input_word, memory),
            native_run_responses,
            guest_run_responses,
        );
    }
}

/// Serves the arithmetic advice of a querier in this process (the native run that records the
/// prover input); the same processor as [`ArithmeticQuery`], as the querier memory abstracts the
/// difference.
#[derive(Default)]
pub struct NativeArithmeticQuery;

impl OracleQueryProcessor for NativeArithmeticQuery {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        ARITHMETIC_QUERY_IDS.to_vec()
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
        mode: RunMode,
        native_run_responses: &mut Vec<u32>,
        guest_run_responses: &mut Vec<u32>,
    ) {
        respond_to_every_target(
            mode,
            process_arithmetic_query(query_id, input_word, memory),
            native_run_responses,
            guest_run_responses,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_utils::TestMemorySource;
    use basic_system::system_functions::modexp::{ModExpAdviceParams, ModExpAdviceParams64};
    use basic_system::system_functions::u256_advice::{
        U256DivRemAdviceQuery, U256WideDivRemAdviceQuery,
    };
    use oracle_provider::{GuestMemory, ZkEENonDeterminismSource};
    use zk_ee::oracle::memory_io::host::NativeQuerierMemory;

    /// The response of `processor` to its querier: the RISC-V guest for guest memory, one in this
    /// process otherwise
    fn respond(
        processor: &mut impl OracleQueryProcessor,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
    ) -> Vec<u32> {
        let mode = if memory.word_size() == size_of::<u32>() {
            RunMode::RiscVRun
        } else {
            RunMode::NativeRunOnly
        };
        let (mut native_run, mut guest_run) = (Vec::new(), Vec::new());
        processor.process_memory_query(
            query_id,
            input_word,
            memory,
            mode,
            &mut native_run,
            &mut guest_run,
        );
        match mode {
            RunMode::RiscVRun => {
                assert!(native_run.is_empty());
                guest_run
            }
            RunMode::NativeRunOnly | RunMode::NativeRunSavingForRiscV => {
                assert!(guest_run.is_empty());
                native_run
            }
        }
    }
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

        let response = respond(
            &mut ArithmeticQuery,
            MODEXP_ADVICE_QUERY_ID,
            PARAMS_ADDR as usize,
            &GuestMemory(&memory),
        );
        split_modexp_response(&response)
    }

    /// The quotient and the remainder of a modexp advice response, as 64-bit limbs
    fn split_modexp_response(response: &[u32]) -> (Vec<u64>, Vec<u64>) {
        let (q_len, r_len) = (response[0] as usize, response[1] as usize);
        assert_eq!(response.len(), 2 + q_len + r_len);
        let limbs = |words: &[u32]| -> Vec<u64> {
            assert!(words.len().is_multiple_of(2));
            words
                .as_chunks::<2>()
                .0
                .iter()
                .map(|[low, high]| u64::from(*low) | (u64::from(*high) << 32))
                .collect()
        };
        (
            limbs(&response[2..2 + q_len]),
            limbs(&response[2 + q_len..]),
        )
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

    /// Runs the modexp advice query of a querier of this process with the request `arg`
    fn run_native_division_query(arg: &ModExpAdviceParams64) -> Vec<u32> {
        // SAFETY: the request and the operands it points to are alive during the call
        let memory = unsafe { NativeQuerierMemory::new() };
        respond(
            &mut NativeArithmeticQuery,
            MODEXP_ADVICE_QUERY_ID,
            core::ptr::from_ref(arg).expose_provenance(),
            &memory,
        )
    }

    #[test]
    fn native_arithmetic_query_processes_valid_query() {
        let dividend = [10u64, 0, 0, 0];
        let modulus = [3u64, 0, 0, 0];
        let arg = ModExpAdviceParams64 {
            op: 0,
            a_ptr: dividend.as_ptr().expose_provenance() as u64,
            a_len: 1,
            b_ptr: 0,
            b_len: 0,
            modulus_ptr: modulus.as_ptr().expose_provenance() as u64,
            modulus_len: 1,
        };

        // lengths in words, then q = 3 and r = 1
        assert_eq!(run_native_division_query(&arg), vec![2, 2, 3, 0, 1, 0]);
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
        let response = respond(
            &mut ArithmeticQuery,
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

        let riscv_output = respond(
            &mut ArithmeticQuery,
            MODEXP_ADVICE_QUERY_ID,
            GUEST_ARG_ADDR as usize,
            &GuestMemory(&memory),
        );

        let host_arg = ModExpAdviceParams64 {
            op: 0,
            a_ptr: dividend.as_mut_ptr().expose_provenance() as u64,
            a_len: DIVIDEND_DIGITS as u64,
            b_ptr: 0,
            b_len: 0,
            modulus_ptr: modulus.as_mut_ptr().expose_provenance() as u64,
            modulus_len: MODULUS_DIGITS as u64,
        };
        let native_output = run_native_division_query(&host_arg);

        assert_eq!(native_output, riscv_output);

        let (quotient, remainder) = split_modexp_response(&native_output);
        assert!(
            quotient.len() > 1,
            "quotient should span multiple u64 limbs"
        );
        assert!(
            remainder.len() > 1,
            "remainder should span multiple u64 limbs"
        );
    }

    #[test]
    #[should_panic]
    fn arithmetic_query_panics_on_null_request() {
        let memory = TestMemorySource::default();
        let _ = respond(
            &mut ArithmeticQuery,
            MODEXP_ADVICE_QUERY_ID,
            0,
            &GuestMemory(&memory),
        );
    }

    #[test]
    #[should_panic]
    fn arithmetic_query_panics_on_misaligned_pointer() {
        let memory = TestMemorySource::default();
        let _ = respond(
            &mut ArithmeticQuery,
            MODEXP_ADVICE_QUERY_ID,
            0x101,
            &GuestMemory(&memory),
        );
    }

    #[test]
    #[should_panic]
    fn native_arithmetic_query_rejects_null_query_pointer() {
        // SAFETY: nothing is read at a null address
        let memory = unsafe { NativeQuerierMemory::new() };
        let _ = respond(
            &mut NativeArithmeticQuery,
            MODEXP_ADVICE_QUERY_ID,
            0,
            &memory,
        );
    }
}
