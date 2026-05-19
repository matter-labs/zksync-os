use basic_bootloader::bootloader::oracle_types::{
    DivRemResponse, ModexpResponse, WideDivRemResponse,
};
use basic_system::system_functions::modexp::{
    ModExpAdviceParams, ModExpAdviceParams64, MODEXP_ADVICE_QUERY_ID,
};
use basic_system::system_functions::u256_advice::{
    U256DivRemAdviceParams, U256DivRemAdviceParams64, U256WideDivRemAdviceParams,
    U256WideDivRemAdviceParams64,
};
use oracle_provider::OracleQueryProcessor;
use oracle_provider::RamPeek;
use zk_ee::internal_error;
use zk_ee::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_WIDE_DIV_REM_ADVICE_QUERY_ID};
use zk_ee::system::errors::internal::InternalError;

use crate::utils::evaluate::{read_memory_as_u64, read_struct};
use crate::{read_host_struct, read_u64_words};

fn u256_div_rem(mut dividend: [u64; 4], mut divisor: [u64; 4]) -> DivRemResponse {
    ruint::algorithms::div(&mut dividend, &mut divisor);
    // Return quotient only (4 limbs), guest derives remainder
    DivRemResponse { quotient: dividend }
}

fn u256_wide_div_rem(
    dividend_lo: [u64; 4],
    dividend_hi: [u64; 4],
    mut divisor: [u64; 4],
) -> WideDivRemResponse {
    let mut dividend = [0u64; 8];
    dividend[..4].copy_from_slice(&dividend_lo);
    dividend[4..].copy_from_slice(&dividend_hi);

    ruint::algorithms::div(&mut dividend, &mut divisor);

    // Return quotient only (8 limbs), no remainder
    WideDivRemResponse {
        quotient_lo: [dividend[0], dividend[1], dividend[2], dividend[3]],
        quotient_hi: [dividend[4], dividend[5], dividend[6], dividend[7]],
    }
}

/// Read a U256 (4 u64 limbs) from guest memory at the given u32 address.
fn read_u256_from_guest(memory: &dyn RamPeek, ptr: u32) -> [u64; 4] {
    let limbs = read_memory_as_u64(memory, ptr, 4).unwrap();
    [limbs[0], limbs[1], limbs[2], limbs[3]]
}

/// Read a U256 (4 u64 limbs) from host process memory at the given u64 address.
fn read_u256_from_host(ptr: u64) -> [u64; 4] {
    let limbs = read_u64_words(ptr, 4);
    [limbs[0], limbs[1], limbs[2], limbs[3]]
}

fn strip_trailing_zeros(v: &mut Vec<u64>) {
    while v.last() == Some(&0) {
        v.pop();
    }
}

fn process_modexp_riscv(arg_ptr: u32, memory: &dyn RamPeek) -> ModexpResponse {
    assert!(arg_ptr.is_multiple_of(4));
    const { assert!(core::mem::align_of::<ModExpAdviceParams>() <= 4) }
    const { assert!(core::mem::size_of::<ModExpAdviceParams>().is_multiple_of(4)) }
    let arg = unsafe { read_struct::<ModExpAdviceParams>(memory, arg_ptr) }.unwrap();

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

    strip_trailing_zeros(&mut n);
    strip_trailing_zeros(&mut d);

    ModexpResponse {
        quotient: n,
        remainder: d,
    }
}

fn process_modexp_native(arg_ptr: u64) -> ModexpResponse {
    let arg: ModExpAdviceParams64 = read_host_struct(arg_ptr);

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

    strip_trailing_zeros(&mut n);
    strip_trailing_zeros(&mut d);

    ModexpResponse {
        quotient: n,
        remainder: d,
    }
}

#[derive(Default)]
pub struct ArithmeticQuery;

impl OracleQueryProcessor for ArithmeticQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![
            MODEXP_ADVICE_QUERY_ID,
            U256_DIV_REM_ADVICE_QUERY_ID,
            U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
        ]
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u8],
        memory: &dyn RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        debug_assert!(self.supports_query_id(query_id));

        if query_id == U256_DIV_REM_ADVICE_QUERY_ID {
            let arg_ptr: u32 = wincode::deserialize(input)
                .map_err(|_| internal_error!("decode u256 div_rem ptr failed"))?;
            assert!(arg_ptr.is_multiple_of(4));
            const { assert!(core::mem::align_of::<U256DivRemAdviceParams>() <= 4) }
            const { assert!(core::mem::size_of::<U256DivRemAdviceParams>().is_multiple_of(4)) }
            let params: U256DivRemAdviceParams = unsafe { read_struct(memory, arg_ptr) }.unwrap();
            let dividend = read_u256_from_guest(memory, params.dividend_ptr);
            let divisor = read_u256_from_guest(memory, params.divisor_ptr);
            let response = u256_div_rem(dividend, divisor);
            return wincode::serialize(&response)
                .map_err(|_| internal_error!("encode div_rem response failed"));
        }

        if query_id == U256_WIDE_DIV_REM_ADVICE_QUERY_ID {
            let arg_ptr: u32 = wincode::deserialize(input)
                .map_err(|_| internal_error!("decode u256 wide div_rem ptr failed"))?;
            assert!(arg_ptr.is_multiple_of(4));
            const { assert!(core::mem::align_of::<U256WideDivRemAdviceParams>() <= 4) }
            const { assert!(core::mem::size_of::<U256WideDivRemAdviceParams>().is_multiple_of(4)) }
            let params: U256WideDivRemAdviceParams =
                unsafe { read_struct(memory, arg_ptr) }.unwrap();
            let dividend_lo = read_u256_from_guest(memory, params.dividend_lo_ptr);
            let dividend_hi = read_u256_from_guest(memory, params.dividend_hi_ptr);
            let divisor = read_u256_from_guest(memory, params.divisor_ptr);
            let response = u256_wide_div_rem(dividend_lo, dividend_hi, divisor);
            return wincode::serialize(&response)
                .map_err(|_| internal_error!("encode wide div_rem response failed"));
        }

        let arg_ptr: u32 =
            wincode::deserialize(input).map_err(|_| internal_error!("decode modexp ptr failed"))?;
        let response = process_modexp_riscv(arg_ptr, memory);
        wincode::serialize(&response).map_err(|_| internal_error!("encode modexp response failed"))
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
        vec![
            MODEXP_ADVICE_QUERY_ID,
            U256_DIV_REM_ADVICE_QUERY_ID,
            U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
        ]
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u8],
        _memory: &dyn RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        debug_assert!(self.supports_query_id(query_id));

        if query_id == U256_DIV_REM_ADVICE_QUERY_ID {
            let arg_ptr: u64 = wincode::deserialize(input)
                .map_err(|_| internal_error!("decode u256 div_rem ptr failed"))?;
            let params: U256DivRemAdviceParams64 = read_host_struct(arg_ptr);
            let dividend = read_u256_from_host(params.dividend_ptr);
            let divisor = read_u256_from_host(params.divisor_ptr);
            let response = u256_div_rem(dividend, divisor);
            return wincode::serialize(&response)
                .map_err(|_| internal_error!("encode div_rem response failed"));
        }

        if query_id == U256_WIDE_DIV_REM_ADVICE_QUERY_ID {
            let arg_ptr: u64 = wincode::deserialize(input)
                .map_err(|_| internal_error!("decode u256 wide div_rem ptr failed"))?;
            let params: U256WideDivRemAdviceParams64 = read_host_struct(arg_ptr);
            let dividend_lo = read_u256_from_host(params.dividend_lo_ptr);
            let dividend_hi = read_u256_from_host(params.dividend_hi_ptr);
            let divisor = read_u256_from_host(params.divisor_ptr);
            let response = u256_wide_div_rem(dividend_lo, dividend_hi, divisor);
            return wincode::serialize(&response)
                .map_err(|_| internal_error!("encode wide div_rem response failed"));
        }

        let arg_ptr: u64 =
            wincode::deserialize(input).map_err(|_| internal_error!("decode modexp ptr failed"))?;
        let response = process_modexp_native(arg_ptr);
        wincode::serialize(&response).map_err(|_| internal_error!("encode modexp response failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_utils::TestMemorySource;
    use oracle_provider::DummyMemorySource;

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

        let input = wincode::serialize(&(PARAMS_ADDR as u32)).unwrap();
        let result_bytes = ArithmeticQuery
            .process(MODEXP_ADVICE_QUERY_ID, &input, &memory)
            .unwrap();
        let response: ModexpResponse = wincode::deserialize(&result_bytes).unwrap();
        (response.quotient, response.remainder)
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

        let input =
            wincode::serialize(&((&arg as *const ModExpAdviceParams64).addr() as u64)).unwrap();
        let result_bytes = NativeArithmeticQuery
            .process(MODEXP_ADVICE_QUERY_ID, &input, &DummyMemorySource)
            .unwrap();
        let response: ModexpResponse = wincode::deserialize(&result_bytes).unwrap();

        assert_eq!(response.quotient, vec![3]);
        assert_eq!(response.remainder, vec![1]);
    }

    #[test]
    fn u256_div_rem_via_native_query() {
        let dividend = [10u64, 0, 0, 0];
        let divisor = [3u64, 0, 0, 0];
        let params = U256DivRemAdviceParams64 {
            dividend_ptr: dividend.as_ptr().addr() as u64,
            divisor_ptr: divisor.as_ptr().addr() as u64,
        };
        let input =
            wincode::serialize(&((&params as *const U256DivRemAdviceParams64).addr() as u64))
                .unwrap();
        let result_bytes = NativeArithmeticQuery
            .process(U256_DIV_REM_ADVICE_QUERY_ID, &input, &DummyMemorySource)
            .unwrap();
        let response: DivRemResponse = wincode::deserialize(&result_bytes).unwrap();
        // Quotient only: 4 limbs
        assert_eq!(response.quotient, [3, 0, 0, 0]);
    }

    #[test]
    fn u256_wide_div_rem_via_native_query() {
        // 35 / 6: q=5
        let dividend_lo = [35u64, 0, 0, 0];
        let dividend_hi = [0u64, 0, 0, 0];
        let divisor = [6u64, 0, 0, 0];
        let params = U256WideDivRemAdviceParams64 {
            dividend_lo_ptr: dividend_lo.as_ptr().addr() as u64,
            dividend_hi_ptr: dividend_hi.as_ptr().addr() as u64,
            divisor_ptr: divisor.as_ptr().addr() as u64,
        };
        let input =
            wincode::serialize(&((&params as *const U256WideDivRemAdviceParams64).addr() as u64))
                .unwrap();
        let result_bytes = NativeArithmeticQuery
            .process(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &input,
                &DummyMemorySource,
            )
            .unwrap();
        let response: WideDivRemResponse = wincode::deserialize(&result_bytes).unwrap();
        // Quotient only: lo=5, hi=0
        assert_eq!(response.quotient_lo[0], 5);
        assert_eq!(response.quotient_lo[1..], [0, 0, 0]);
        assert_eq!(response.quotient_hi, [0, 0, 0, 0]);
    }

    #[test]
    fn u256_wide_div_rem_large_dividend() {
        // 2^256 / (2^128 + 1): q = 2^128 - 1
        let dividend_lo = [0u64, 0, 0, 0];
        let dividend_hi = [1u64, 0, 0, 0];
        let divisor = [1u64, 0, 1, 0];
        let params = U256WideDivRemAdviceParams64 {
            dividend_lo_ptr: dividend_lo.as_ptr().addr() as u64,
            dividend_hi_ptr: dividend_hi.as_ptr().addr() as u64,
            divisor_ptr: divisor.as_ptr().addr() as u64,
        };
        let input =
            wincode::serialize(&((&params as *const U256WideDivRemAdviceParams64).addr() as u64))
                .unwrap();
        let result_bytes = NativeArithmeticQuery
            .process(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &input,
                &DummyMemorySource,
            )
            .unwrap();
        let response: WideDivRemResponse = wincode::deserialize(&result_bytes).unwrap();
        assert_eq!(response.quotient_lo[0], u64::MAX);
        assert_eq!(response.quotient_lo[1], u64::MAX);
        assert_eq!(response.quotient_lo[2..], [0, 0]);
        assert_eq!(response.quotient_hi, [0, 0, 0, 0]);
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

        let riscv_input = wincode::serialize(&(GUEST_ARG_ADDR as u32)).unwrap();
        let riscv_result_bytes = ArithmeticQuery
            .process(MODEXP_ADVICE_QUERY_ID, &riscv_input, &memory)
            .unwrap();
        let riscv_response: ModexpResponse = wincode::deserialize(&riscv_result_bytes).unwrap();

        let host_arg = ModExpAdviceParams64 {
            op: 0,
            a_ptr: dividend.as_mut_ptr().addr() as u64,
            a_len: DIVIDEND_DIGITS as u64,
            b_ptr: 0,
            b_len: 0,
            modulus_ptr: modulus.as_mut_ptr().addr() as u64,
            modulus_len: MODULUS_DIGITS as u64,
        };
        let native_input =
            wincode::serialize(&((&host_arg as *const ModExpAdviceParams64).addr() as u64))
                .unwrap();
        let native_result_bytes = NativeArithmeticQuery
            .process(MODEXP_ADVICE_QUERY_ID, &native_input, &DummyMemorySource)
            .unwrap();
        let native_response: ModexpResponse = wincode::deserialize(&native_result_bytes).unwrap();

        assert_eq!(native_response.quotient, riscv_response.quotient);
        assert_eq!(native_response.remainder, riscv_response.remainder);
        assert!(
            native_response.quotient.len() > 1,
            "quotient should span multiple u64 limbs"
        );
        assert!(
            native_response.remainder.len() > 1,
            "remainder should span multiple u64 limbs"
        );
    }

    #[test]
    #[should_panic]
    fn arithmetic_query_panics_on_misaligned_pointer() {
        let memory = TestMemorySource::default();
        let input = wincode::serialize(&(0x101u32)).unwrap();
        let _ = ArithmeticQuery.process(MODEXP_ADVICE_QUERY_ID, &input, &memory);
    }

    #[test]
    #[should_panic]
    fn native_arithmetic_query_rejects_null_query_pointer() {
        let input = wincode::serialize(&(0u64)).unwrap();
        let _ = NativeArithmeticQuery.process(MODEXP_ADVICE_QUERY_ID, &input, &DummyMemorySource);
    }
}
