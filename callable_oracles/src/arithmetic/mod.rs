use basic_system::system_functions::modexp::advice::bigint::ModexpResponse;
use basic_system::system_functions::modexp::MODEXP_ADVICE_QUERY_ID;
use oracle_provider::OracleQueryProcessor;
use oracle_provider::RamPeek;
use zk_ee::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_WIDE_DIV_REM_ADVICE_QUERY_ID};
use zk_ee::oracle::word_layout::WordLayout;
use zk_ee::system::errors::internal::InternalError;

/// Decode WordLayout-encoded u32 words back into a typed value.
fn decode_input<T: WordLayout>(input: &[u32]) -> T {
    let mut cursor = 0;
    T::read_words(&mut || {
        let w = input.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        w
    })
}

fn u256_div_rem_output(mut dividend: [u64; 4], mut divisor: [u64; 4]) -> Vec<u32> {
    ruint::algorithms::div(&mut dividend, &mut divisor);

    // Return quotient only (4 limbs) as u32 words via WordLayout
    let mut result = Vec::new();
    dividend.write_words(&mut |w| result.push(w));
    result
}

fn u256_wide_div_rem_output(
    dividend_lo: [u64; 4],
    dividend_hi: [u64; 4],
    mut divisor: [u64; 4],
) -> Vec<u32> {
    let mut dividend = [0u64; 8];
    dividend[..4].copy_from_slice(&dividend_lo);
    dividend[4..].copy_from_slice(&dividend_hi);

    ruint::algorithms::div(&mut dividend, &mut divisor);

    // Return quotient only (8 limbs) as u32 words via WordLayout
    let mut result = Vec::new();
    dividend.write_words(&mut |w| result.push(w));
    result
}

fn strip_trailing_zeros_u64(v: &mut Vec<u64>) {
    while v.last() == Some(&0) {
        v.pop();
    }
}

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

fn process_modexp_query(input: &[u32]) -> Vec<u32> {
    let mut cursor = 0;
    let mut read = || {
        let w = input.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        w
    };
    let _op: u32 = WordLayout::read_words(&mut read);
    let a_words: Vec<u32> = WordLayout::read_words(&mut read);
    let modulus_words: Vec<u32> = WordLayout::read_words(&mut read);

    let mut n: Vec<u64> = a_words
        .chunks(2)
        .map(|c| {
            let lo = c[0] as u64;
            let hi = if c.len() > 1 { c[1] as u64 } else { 0 };
            lo | (hi << 32)
        })
        .collect();
    let mut d: Vec<u64> = modulus_words
        .chunks(2)
        .map(|c| {
            let lo = c[0] as u64;
            let hi = if c.len() > 1 { c[1] as u64 } else { 0 };
            lo | (hi << 32)
        })
        .collect();

    assert!(!d.is_empty());
    ruint::algorithms::div(&mut n, &mut d);

    strip_trailing_zeros_u64(&mut n);
    strip_trailing_zeros_u64(&mut d);

    let response = ModexpResponse {
        quotient: n,
        remainder: d,
    };
    let mut result = Vec::new();
    response.write_words(&mut |w| result.push(w));
    result
}

/// Unified arithmetic query processor. Handles both RISC-V and native queries
/// since inputs arrive WordLayout-encoded (no pointer dereference needed).
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
        input: &[u32],
        _memory: &dyn RamPeek,
    ) -> Result<Vec<u32>, InternalError> {
        debug_assert!(self.supports_query_id(query_id));

        if query_id == U256_DIV_REM_ADVICE_QUERY_ID {
            // Input: [u64; 8] WordLayout-encoded = 16 u32 words (dividend 4 limbs + divisor 4 limbs)
            let limbs: [u64; 8] = decode_input(input);
            let dividend: [u64; 4] = limbs[..4].try_into().unwrap();
            let divisor: [u64; 4] = limbs[4..].try_into().unwrap();
            return Ok(u256_div_rem_output(dividend, divisor));
        }

        if query_id == U256_WIDE_DIV_REM_ADVICE_QUERY_ID {
            // Input: [u64; 12] WordLayout-encoded = 24 u32 words (dividend_lo + dividend_hi + divisor)
            let limbs: [u64; 12] = decode_input(input);
            let dividend_lo: [u64; 4] = limbs[..4].try_into().unwrap();
            let dividend_hi: [u64; 4] = limbs[4..8].try_into().unwrap();
            let divisor: [u64; 4] = limbs[8..].try_into().unwrap();
            return Ok(u256_wide_div_rem_output(dividend_lo, dividend_hi, divisor));
        }

        // Modexp query
        Ok(process_modexp_query(input))
    }
}

/// Backward-compatible alias. Since inputs now arrive WordLayout-encoded,
/// there is no distinction between RISC-V and native processing.
#[derive(Default)]
pub struct NativeArithmeticQuery;

impl OracleQueryProcessor for NativeArithmeticQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        ArithmeticQuery.supported_query_ids()
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u32],
        memory: &dyn RamPeek,
    ) -> Result<Vec<u32>, InternalError> {
        ArithmeticQuery.process(query_id, input, memory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use oracle_provider::DummyMemorySource;

    #[test]
    fn u256_div_rem_basic() {
        // 10 / 3 = q=3, r=0 (in U256 representation)
        let dividend = [10u64, 0, 0, 0];
        let divisor = [3u64, 0, 0, 0];
        let mut input_words = Vec::new();
        let combined: [u64; 8] = {
            let mut arr = [0u64; 8];
            arr[..4].copy_from_slice(&dividend);
            arr[4..].copy_from_slice(&divisor);
            arr
        };
        combined.write_words(&mut |w| input_words.push(w));

        let output = ArithmeticQuery
            .process(
                U256_DIV_REM_ADVICE_QUERY_ID,
                &input_words,
                &DummyMemorySource,
            )
            .unwrap();

        // Output is [u64; 4] = 8 u32 words: quotient = 3
        let result: [u64; 4] = decode_input(&output);
        assert_eq!(result, [3, 0, 0, 0]);
    }

    #[test]
    fn u256_wide_div_rem_basic() {
        // 35 / 6: q=5
        let dividend_lo = [35u64, 0, 0, 0];
        let dividend_hi = [0u64, 0, 0, 0];
        let divisor = [6u64, 0, 0, 0];
        let mut input_words = Vec::new();
        let combined: [u64; 12] = {
            let mut arr = [0u64; 12];
            arr[..4].copy_from_slice(&dividend_lo);
            arr[4..8].copy_from_slice(&dividend_hi);
            arr[8..].copy_from_slice(&divisor);
            arr
        };
        combined.write_words(&mut |w| input_words.push(w));

        let output = ArithmeticQuery
            .process(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &input_words,
                &DummyMemorySource,
            )
            .unwrap();

        // Output is [u64; 8] = 16 u32 words
        let result: [u64; 8] = decode_input(&output);
        assert_eq!(result[0], 5);
        assert_eq!(&result[1..], &[0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn u256_wide_div_rem_large_dividend() {
        // 2^256 / (2^128 + 1): q = 2^128 - 1
        let dividend_lo = [0u64, 0, 0, 0];
        let dividend_hi = [1u64, 0, 0, 0];
        let divisor = [1u64, 0, 1, 0];
        let mut input_words = Vec::new();
        let combined: [u64; 12] = {
            let mut arr = [0u64; 12];
            arr[..4].copy_from_slice(&dividend_lo);
            arr[4..8].copy_from_slice(&dividend_hi);
            arr[8..].copy_from_slice(&divisor);
            arr
        };
        combined.write_words(&mut |w| input_words.push(w));

        let output = ArithmeticQuery
            .process(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &input_words,
                &DummyMemorySource,
            )
            .unwrap();

        let result: [u64; 8] = decode_input(&output);
        assert_eq!(result[0], u64::MAX);
        assert_eq!(result[1], u64::MAX);
        assert_eq!(&result[2..], &[0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn modexp_basic_division() {
        // Modexp: 10 / 3 = q=3, r=1
        // Encode as ModexpReductionInput { op: 0, a_words: [10, 0, 0, 0, 0, 0, 0, 0], modulus_words: [3, 0, 0, 0, 0, 0, 0, 0] }
        let mut input_words = Vec::new();
        // op: u32
        (0u32).write_words(&mut |w| input_words.push(w));
        // a_words: Vec<u32> - 1 digit = 8 u32 words
        let a: Vec<u32> = vec![10, 0, 0, 0, 0, 0, 0, 0];
        a.write_words(&mut |w| input_words.push(w));
        // modulus_words: Vec<u32>
        let m: Vec<u32> = vec![3, 0, 0, 0, 0, 0, 0, 0];
        m.write_words(&mut |w| input_words.push(w));

        let output = ArithmeticQuery
            .process(MODEXP_ADVICE_QUERY_ID, &input_words, &DummyMemorySource)
            .unwrap();

        let mut cursor = 0;
        let response = ModexpResponse::read_words(&mut || {
            let w = output[cursor];
            cursor += 1;
            w
        });
        assert_eq!(response.quotient, vec![3u64]);
        assert_eq!(response.remainder, vec![1u64]);
    }

    #[test]
    #[should_panic]
    fn modexp_division_by_zero() {
        let mut input_words = Vec::new();
        (0u32).write_words(&mut |w| input_words.push(w));
        let a: Vec<u32> = vec![10, 0, 0, 0, 0, 0, 0, 0];
        a.write_words(&mut |w| input_words.push(w));
        let m: Vec<u32> = vec![0, 0, 0, 0, 0, 0, 0, 0];
        m.write_words(&mut |w| input_words.push(w));

        let _ = ArithmeticQuery.process(MODEXP_ADVICE_QUERY_ID, &input_words, &DummyMemorySource);
    }
}
