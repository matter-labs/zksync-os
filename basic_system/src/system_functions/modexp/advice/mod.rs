use alloc::vec::Vec;
use bigint::ModexpAdvisor;
use core::alloc::Allocator;

mod bigint;
mod exponent;
mod single_digit;
mod u256;

use self::bigint::BigintRepr;
use crate::system_functions::modexp::strip_leading_zeroes;

use zk_ee::system::logger::Logger;
#[cfg(feature = "testing")]
use zk_ee::system::logger::NullLogger;

pub(super) fn modexp<O: zk_ee::oracle::IOOracle, L: Logger, A: Allocator + Clone>(
    base: &[u8],
    exp: &[u8],
    modulus: &[u8],
    oracle: &mut O,
    _logger: &mut L,
    allocator: A,
) -> Vec<u8, A> {
    let mut advisor = self::bigint::OracleAdvisor { inner: oracle };

    modexp_inner::<L, A>(base, exp, modulus, _logger, &mut advisor, allocator)
}

/// Same logic as the delegated modexp used for proving, but
/// with a naive advisor for testing purposes.
#[cfg(feature = "testing")]
pub fn delegated_modexp_with_naive_advisor(base: &[u8], exp: &[u8], modulus: &[u8]) -> Vec<u8> {
    use std::alloc::Global;
    let mut advisor = bigint::naive_advisor::NaiveAdvisor;
    let mut logger = NullLogger;
    modexp_inner::<NullLogger, Global>(base, exp, modulus, &mut logger, &mut advisor, Global)
}

fn modexp_inner<L: Logger, A: Allocator + Clone>(
    base: &[u8],
    exp: &[u8],
    modulus: &[u8],
    _logger: &mut L,
    advisor: &mut impl ModexpAdvisor,
    allocator: A,
) -> Vec<u8, A> {
    let modulus_digits = strip_leading_zeroes(modulus);
    if modulus_digits.is_empty() {
        return Vec::new_in(allocator);
    }
    if modulus_digits.len() <= 32 {
        // one digit: the U256 path
        let mut padded = [0u8; 32];
        padded[32 - modulus_digits.len()..].copy_from_slice(modulus_digits);
        let m = ::u256::U256::from_be_bytes(&padded);
        if m.is_one() {
            // it is base ^ exponent mod 1 == 0 in all the cases
            return Vec::new_in(allocator);
        }
        return single_digit::modexp(base, exp, &m, advisor, allocator);
    }

    let m = BigintRepr::from_big_endian_with_double_capacity(&modulus, allocator.clone());
    debug_assert!(m.digits > 1);
    {
        let min_capacity = m.capacity();
        let x = BigintRepr::from_big_endian_with_double_capacity_or_min_capacity(
            &base,
            min_capacity,
            allocator.clone(),
        );
        let x = x.modpow(&exp, m, advisor, allocator.clone());
        x.to_big_endian(allocator)
    }
}

#[cfg(test)]
mod test {
    use std::alloc::Global;

    use super::bigint::naive_advisor::NaiveAdvisor;
    use super::*;
    use num_bigint::BigUint;
    use num_traits::Zero;

    fn invoke_precompile_with_advisor(
        modulus: &[u8],
        base: &[u8],
        exp: &[u8],
        advisor: &mut impl ModexpAdvisor,
    ) -> Vec<u8> {
        let mut logger = zk_ee::system::logger::NullLogger;
        super::modexp_inner(base, exp, modulus, &mut logger, advisor, Global)
    }

    fn invoke_precompile_no_prepadding(modulus: &[u8], base: &[u8], exp: &[u8]) -> Vec<u8> {
        let mut advisor = NaiveAdvisor;
        invoke_precompile_with_advisor(modulus, base, exp, &mut advisor)
    }

    #[derive(Clone)]
    struct ReductionAdvice {
        quotient_digits: std::vec::Vec<u64>,
        remainder_digits: std::vec::Vec<u64>,
    }

    fn write_bigint_digits<A: core::alloc::Allocator + Clone>(
        digits: &[u64],
        dst: &mut BigintRepr<A>,
    ) {
        unsafe {
            let dst_capacity = dst.clear_as_capacity_mut();
            for (dst_digit, src_digit) in dst_capacity.iter_mut().zip(digits.iter()) {
                let dst_ptr: *mut u64 = dst_digit.as_mut_ptr().cast::<[u64; 4]>().cast();
                dst_ptr.add(0).write(*src_digit);
                dst_ptr.add(1).write(0);
                dst_ptr.add(2).write(0);
                dst_ptr.add(3).write(0);
            }
            dst.set_num_digits(digits.len());
        }
    }

    struct MaliciousReductionAdvisor {
        responses: std::vec::Vec<ReductionAdvice>,
        next_response: usize,
    }

    impl MaliciousReductionAdvisor {
        fn new(responses: std::vec::Vec<ReductionAdvice>) -> Self {
            Self {
                responses,
                next_response: 0,
            }
        }

        fn assert_consumed(&self) {
            assert_eq!(self.next_response, self.responses.len());
        }
    }

    impl ModexpAdvisor for MaliciousReductionAdvisor {
        fn get_reduction_op_advice<A: core::alloc::Allocator + Clone>(
            &mut self,
            _a: &BigintRepr<A>,
            _m: &BigintRepr<A>,
            quotient_dst: &mut BigintRepr<A>,
            remainder_dst: &mut BigintRepr<A>,
        ) {
            let advice = self
                .responses
                .get(self.next_response)
                .cloned()
                .expect("unexpected modexp advice query");
            self.next_response += 1;

            write_bigint_digits(&advice.quotient_digits, quotient_dst);
            write_bigint_digits(&advice.remainder_digits, remainder_dst);
        }

        fn wide_quotient(
            &mut self,
            lo: &::u256::U256,
            hi: &::u256::U256,
            modulus: &::u256::U256,
        ) -> (::u256::U256, ::u256::U256) {
            super::bigint::naive_wide_quotient(lo, hi, modulus)
        }
    }

    // #[ignore = "depends on init and features"]
    #[test]
    fn test_on_vector() {
        // let test = Test {
        //     input: "\
        //     0000000000000000000000000000000000000000000000000000000000000040\
        //     0000000000000000000000000000000000000000000000000000000000000001\
        //     0000000000000000000000000000000000000000000000000000000000000040\
        //     e09ad9675465c53a109fac66a445c91b292d2bb2c5268addb30cd82f80fcb003\
        //     3ff97c80a5fc6f39193ae969c6ede6710a6b7ac27078a06d90ef1c72e5c85fb5\
        //     02fc9e1f6beb81516545975218075ec2af118cd8798df6e08a147c60fd6095ac\
        //     2bb02c2908cf4dd7c81f11c289e4bce98f3553768f392a80ce22bf5c4f4a248c\
        //     6b",
        //     expected: "60008f1614cc01dcfb6bfb09c625cf90b47d4468db81b5f8b7a39d42f332eab9b2da8f2d95311648a8f243f4bb13cfb3d8f7f2a3c014122ebb3ed41b02783adc",
        //     name: "nagydani_1_square",
        //     precompile_id: "0000000000000000000000000000000000000005",
        // };

        let base = hex::decode("e09ad9675465c53a109fac66a445c91b292d2bb2c5268addb30cd82f80fcb0033ff97c80a5fc6f39193ae969c6ede6710a6b7ac27078a06d90ef1c72e5c85fb5").unwrap();
        assert_eq!(base.len(), 64);

        let exp = hex::decode("02").unwrap();
        assert_eq!(exp.len(), 1);

        let modulus = hex::decode("fc9e1f6beb81516545975218075ec2af118cd8798df6e08a147c60fd6095ac2bb02c2908cf4dd7c81f11c289e4bce98f3553768f392a80ce22bf5c4f4a248c6b").unwrap();
        assert_eq!(modulus.len(), 64);

        let expected = hex::decode("60008f1614cc01dcfb6bfb09c625cf90b47d4468db81b5f8b7a39d42f332eab9b2da8f2d95311648a8f243f4bb13cfb3d8f7f2a3c014122ebb3ed41b02783adc").unwrap();
        assert_eq!(expected.len(), 64);

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        assert_eq!(&output, &expected);
    }

    #[test]
    fn test_zero_output() {
        let base = hex::decode("5442ddc2b70f66c1f6d2b296c0a875be7eddd0a80958cbc7425f1899ccf90511a5c318226e48ee23f130b44dc17a691ce66be5da18b85ed7943535b205aa125e9f59294a00f05155c23e97dac6b3a00b0c63c8411bf815fc183b420b4d9dc5f715040d5c").unwrap();
        assert_eq!(base.len(), 0x64);

        let exp = hex::decode("60957f52d334b843197adec58c131c907cd96059fc5adce9dda351b5df3d666fcf3eb63c46851c1816e323f2119ebdf5ef35").unwrap();
        assert!(exp.len() < 0x64);
        let mut exp_prepadded = vec![0u8; 0x64 - exp.len()];
        exp_prepadded.extend(exp);
        assert_eq!(exp_prepadded.len(), 0x64);

        let modulus = vec![0u8; 100];

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp_prepadded);

        assert!(output.is_empty());
    }

    #[test]
    fn test_3() {
        // Test {
        //     input: "\
        //     0000000000000000000000000000000000000000000000000000000000000001\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     03\
        //     fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
        //     ffffffffffffffffffffffffffffffffffffffffff2f",
        //     expected: "162ead82cadefaeaf6e9283248fdf2f2845f6396f6f17c4d5a39f820b6f6b5f9",
        //     name: "eth_tests_create2callPrecompiles_test0_berlin",
        //     precompile_id: "0000000000000000000000000000000000000005",
        // }

        let base = hex::decode("03").unwrap();
        assert_eq!(base.len(), 1);

        let exp = hex::decode("fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e")
            .unwrap();
        assert_eq!(exp.len(), 32);

        let encoding = "ffffffffffffffffffffffffffffffffffffffffff2f";
        let mut modulus = hex::decode(encoding).unwrap();
        modulus.resize(32, 0u8);

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected =
            hex::decode("162ead82cadefaeaf6e9283248fdf2f2845f6396f6f17c4d5a39f820b6f6b5f9")
                .unwrap();

        assert_eq!(output, expected);
    }

    #[test]
    fn test_4() {
        // 0000000000000000000000000000000000000000000000000000000000000020
        // 0000000000000000000000000000000000000000000000000000000000000080
        // 0000000000000000000000000000000000000000000000000000000000000020
        // 6ea6c150792130fbfb05b72aacba79157f9b86e05c975cb1585e68fb663801da
        // 0000000000000000000000000000000000000000000000000000000000000000
        // 0000000000000000000000000000000000000000000000000000000000000000
        // 0000000000000000000000000000000000000000000000000000000000000000
        // 000000000000000000000000000000000000000000000000000000000000ffff
        // 148f0b9e252c56e138f8c65a832ebca75241a386a918c14f466fb84a22f8b771

        let base = hex::decode("6ea6c150792130fbfb05b72aacba79157f9b86e05c975cb1585e68fb663801da")
            .unwrap();
        assert_eq!(base.len(), 32);

        let exp = hex::decode("000000000000000000000000000000000000000000000000000000000000ffff")
            .unwrap();

        let modulus =
            hex::decode("148f0b9e252c56e138f8c65a832ebca75241a386a918c14f466fb84a22f8b771")
                .unwrap();
        assert_eq!(modulus.len(), 32);

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected =
            hex::decode("08d8fab720b60be2e3af8437e15e467c625cd8704c2382449e7a50437355c6be")
                .unwrap();

        assert_eq!(output, expected);
    }

    #[test]
    fn test_5() {
        // 0^1 mod 2

        let base = hex::decode("00").unwrap();

        let exp = hex::decode("01").unwrap();

        let modulus = hex::decode("02").unwrap();

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected = hex::decode("").unwrap();

        assert_eq!(output, expected);
    }

    #[test]
    fn test_6() {
        // 0^0 mod 2

        let base = hex::decode("00").unwrap();

        let exp = hex::decode("00").unwrap();

        let modulus = hex::decode("02").unwrap();

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();

        assert_eq!(output, expected);
    }

    #[test]
    fn test_7() {
        // 3^2 mod 1

        let base = hex::decode("03").unwrap();

        let exp = hex::decode("02").unwrap();

        let modulus = hex::decode("01").unwrap();

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected = hex::decode("").unwrap();

        assert_eq!(output, expected);
    }

    #[test]
    fn test_8() {
        // 0^0 mod 1

        let base = hex::decode("00").unwrap();

        let exp = hex::decode("00").unwrap();

        let modulus = hex::decode("01").unwrap();

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected = hex::decode("").unwrap();

        assert_eq!(output, expected);
    }

    /// A small deterministic generator (xorshift64*) for the randomized comparisons
    struct Rng(u64);

    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn bytes(&mut self, len: usize) -> std::vec::Vec<u8> {
            (0..len).map(|_| self.next_u64() as u8).collect()
        }
    }

    fn assert_matches_reference(base: &[u8], exp: &[u8], modulus: &[u8]) {
        let output = invoke_precompile_no_prepadding(modulus, base, exp);
        let modulus_big = BigUint::from_bytes_be(modulus);
        let expected = if modulus_big.is_zero() {
            std::vec::Vec::new()
        } else {
            let result =
                BigUint::from_bytes_be(base).modpow(&BigUint::from_bytes_be(exp), &modulus_big);
            if result.is_zero() {
                std::vec::Vec::new()
            } else {
                result.to_bytes_be()
            }
        };
        assert_eq!(
            BigUint::from_bytes_be(&output),
            BigUint::from_bytes_be(&expected),
            "base {base:02x?} exp {exp:02x?} modulus {modulus:02x?}"
        );
    }

    /// The single-digit modulus path against the reference: bases of up to 4 digits (the
    /// Horner reduction), exponents around the window threshold, moduli of every size up to
    /// 32 bytes including even and small ones.
    #[test]
    fn single_digit_modulus_matches_reference() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for round in 0..400 {
            let modulus_len = 1 + (rng.next_u64() as usize) % 32;
            let mut modulus = rng.bytes(modulus_len);
            if round % 7 == 0 {
                // a small modulus: two-digit quotients in every step
                modulus = vec![1 + (rng.next_u64() as u8) % 16];
            }
            let base_len = (rng.next_u64() as usize) % 130;
            let base = rng.bytes(base_len);
            let exp_len = match round % 4 {
                0 => (rng.next_u64() as usize) % 8,
                1 => 8,
                2 => 9 + (rng.next_u64() as usize) % 24,
                _ => 32 + (rng.next_u64() as usize) % 40,
            };
            let mut exp = rng.bytes(exp_len);
            if round % 11 == 0 {
                // sparse exponents: zero windows and short runs
                for byte in exp.iter_mut() {
                    *byte &= 0x11;
                }
            }
            assert_matches_reference(&base, &exp, &modulus);
        }
    }

    /// The special values of the single-digit path
    #[test]
    fn single_digit_modulus_special_cases() {
        let m = vec![0xffu8; 32];
        // 0^0 = 1, 0^e = 0, 1^e = 1, base = modulus, base = modulus + 1, exp = 0 for a large base
        assert_matches_reference(&[], &[], &m);
        assert_matches_reference(&[0, 0], &[5], &m);
        assert_matches_reference(&[1], &[0xff; 40], &m);
        assert_matches_reference(&m, &[3], &m);
        let mut m_plus_one = vec![1u8];
        m_plus_one.extend(std::iter::repeat_n(0u8, 32));
        assert_matches_reference(&m_plus_one, &[3], &m);
        assert_matches_reference(&[0xab; 100], &[], &m);
        // modulus 2 and an even modulus with a zero-absorbing base
        assert_matches_reference(&[7], &[0xff; 9], &[2]);
        assert_matches_reference(&[2], &[0xff; 9], &[0, 0, 0x40]);
        // leading zero bytes of the modulus do not change the path
        let mut padded = vec![0u8; 40];
        padded.extend_from_slice(&m);
        assert_matches_reference(&[0xab; 33], &[0xcd; 9], &padded);
    }

    /// The multi-digit modulus path against the reference: 2 to 5 digit moduli, bases up to
    /// 8 digits (the initial reduction) and exponents on both sides of the window threshold
    #[test]
    fn multi_digit_modulus_matches_reference() {
        let mut rng = Rng(0xD1B5_4A32_D192_ED03);
        for round in 0..60 {
            let modulus_len = 33 + (rng.next_u64() as usize) % 128;
            let modulus = rng.bytes(modulus_len);
            let base_len = (rng.next_u64() as usize) % 260;
            let base = rng.bytes(base_len);
            let exp_len = match round % 3 {
                0 => (rng.next_u64() as usize) % 8,
                1 => 8 + (rng.next_u64() as usize) % 4,
                _ => 12 + (rng.next_u64() as usize) % 10,
            };
            let exp = rng.bytes(exp_len);
            assert_matches_reference(&base, &exp, &modulus);
        }
        // special values
        let m = vec![0xffu8; 64];
        assert_matches_reference(&[], &[], &m);
        assert_matches_reference(&[0], &[5], &m);
        assert_matches_reference(&[1], &[0xff; 40], &m);
        assert_matches_reference(&m, &[3], &m);
        assert_matches_reference(&[0xab; 100], &[], &m);
        // an even modulus with a zero-absorbing base: 2^512 mod 2^500
        let mut even = vec![0u8; 63];
        even.insert(0, 0x10);
        assert_matches_reference(&[2], &[0xff, 0xff], &even);
    }

    #[test]
    fn test_modexp_delegation_add_overflow_regression() {
        let base = vec![255u8];
        let exp = vec![48, 255, 128, 209];
        let modulus = vec![
            214, 2, 245, 148, 60, 16, 255, 255, 255, 255, 255, 255, 255, 12, 0, 0, 0, 216, 112,
            144, 135, 112, 173, 239, 243, 255, 194, 78, 78, 1, 46, 10, 211, 128, 5, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let base_big = BigUint::from_bytes_be(&base);
        let exp_big = BigUint::from_bytes_be(&exp);
        let modulus_big = BigUint::from_bytes_be(&modulus);
        let expected = base_big.modpow(&exp_big, &modulus_big).to_bytes_be();

        assert_eq!(output, expected);
    }

    #[test]
    #[should_panic]
    fn test_modexp_delegation_malicious_advisor_truncates_reconstruction_regression() {
        let base = hex::decode("02").unwrap();
        let exp = hex::decode("02").unwrap();
        let modulus = vec![0xff; 32];

        let honest_output = invoke_precompile_no_prepadding(&modulus, &base, &exp);
        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000004")
                .unwrap();
        assert_eq!(honest_output, expected);

        // First response performs the initial reduction honestly: 2 = 0 * m + 2.
        // Second response forges the square-step reduction as
        // 4 = low_512((2^256 + 1) * m + 5) while the dropped carry is 1 at the third limb.
        let mut advisor = MaliciousReductionAdvisor::new(vec![
            ReductionAdvice {
                quotient_digits: vec![],
                remainder_digits: vec![2],
            },
            ReductionAdvice {
                quotient_digits: vec![1, 1],
                remainder_digits: vec![5],
            },
        ]);
        let _output = invoke_precompile_with_advisor(&modulus, &base, &exp, &mut advisor);
        advisor.assert_consumed();
    }
}
