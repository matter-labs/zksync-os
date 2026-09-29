//! modexp for a modulus of one 256-bit digit (the common shape): residues are `U256`s and a
//! modular multiplication is one 512-bit product, one advised quotient and its check
//! (`u256_advice::reduce_product_of_residues_with_quotient`), with no buffers to allocate or
//! rotate.

use super::bigint::ModexpAdvisor;
use super::exponent::{exponentiate, ModMul};
use crate::system_functions::modexp::strip_leading_zeroes;
use crate::system_functions::u256_advice::{
    clone_into_slot, reduce_product_of_residues_with_quotient, verify_wide_div_rem_hint,
};
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::mem::MaybeUninit;
use u256::U256;

struct U256ModMul<'a, Adv: ModexpAdvisor> {
    modulus: &'a U256,
    advisor: &'a mut Adv,
}

impl<Adv: ModexpAdvisor> ModMul for U256ModMul<'_, Adv> {
    type Elem = U256;

    fn placeholder(&mut self) -> U256 {
        U256::zero()
    }

    fn clone_elem(&mut self, x: &U256) -> U256 {
        x.clone()
    }

    fn is_zero(&self, x: &U256) -> bool {
        x.is_zero()
    }

    #[inline(always)]
    fn square_assign(&mut self, x: &mut U256) {
        // the product needs the multiplier as a separate operand
        let mut y = MaybeUninit::uninit();
        let y = clone_into_slot(&mut y, x);
        self.mul_assign(x, y);
    }

    #[inline(always)]
    fn mul_assign(&mut self, x: &mut U256, y: &U256) {
        // residues: `exponentiate` only multiplies the reduced base and checked remainders
        debug_assert!(*x < *self.modulus && *y < *self.modulus);
        // the product lands in (x, hi)
        let mut hi = MaybeUninit::uninit();
        let hi = clone_into_slot(&mut hi, x);
        x.widening_mul_assign_into(hi, y);
        let (q_lo, q_hi) = self.advisor.wide_quotient(x, hi, self.modulus);
        reduce_product_of_residues_with_quotient(x, hi, self.modulus, q_lo, q_hi);
    }
}

/// `base^exp mod modulus` as big-endian bytes, for `modulus >= 2`
pub(crate) fn modexp<Adv: ModexpAdvisor, A: Allocator>(
    base: &[u8],
    exp: &[u8],
    modulus: &U256,
    advisor: &mut Adv,
    allocator: A,
) -> Vec<u8, A> {
    debug_assert!(!modulus.is_zero() && !modulus.is_one());

    let base = reduce_base(strip_leading_zeroes(base), modulus, advisor);
    let exp = strip_leading_zeroes(exp);

    let result = if exp.is_empty() {
        // base^0 = 1, including 0^0
        U256::one()
    } else if base.is_zero() || base.is_one() {
        base
    } else {
        let mut arithmetic = U256ModMul { modulus, advisor };
        exponentiate(&mut arithmetic, &base, exp)
    };

    // the multi-digit path's convention: no bytes for zero, the whole digit otherwise (the
    // caller pads or truncates the output to the modulus length)
    let mut out = Vec::with_capacity_in(32, allocator);
    if !result.is_zero() {
        out.extend_from_slice(&result.to_be_bytes());
    }
    out
}

/// `base mod modulus` for a big-endian `base` of any length, digit by digit from the top
/// (Horner): `acc = (acc * 2^256 + digit) mod modulus`, each step a 512-bit division with
/// an advised quotient. A digit already below the modulus with a zero accumulator (a base
/// of at most 32 bytes below the modulus, the common case) needs no advice.
fn reduce_base<Adv: ModexpAdvisor>(base: &[u8], modulus: &U256, advisor: &mut Adv) -> U256 {
    let mut acc = U256::zero();
    let (head, digits) = base.as_rchunks::<32>();
    if !head.is_empty() {
        let mut padded = [0u8; 32];
        padded[32 - head.len()..].copy_from_slice(head);
        reduce_digit(&mut acc, &padded, modulus, advisor);
    }
    for digit in digits {
        reduce_digit(&mut acc, digit, modulus, advisor);
    }
    acc
}

fn reduce_digit<Adv: ModexpAdvisor>(
    acc: &mut U256,
    digit: &[u8; 32],
    modulus: &U256,
    advisor: &mut Adv,
) {
    let mut lo = U256::from_be_bytes(digit);
    if acc.is_zero() {
        // lo < modulus: the subtraction on a scratch borrows
        let mut scratch = lo.clone();
        if scratch.overflowing_sub_assign(modulus) {
            *acc = lo;
            return;
        }
    }
    let (q_lo, q_hi) = advisor.wide_quotient(&lo, acc, modulus);
    assert!(
        verify_wide_div_rem_hint(&mut lo, acc, modulus, &q_lo, &q_hi),
        "modexp base reduction hint: wrong quotient"
    );
    *acc = lo;
}

#[cfg(test)]
mod tests {
    use super::super::bigint::{naive_wide_quotient, BigintRepr, OracleAdvisor};
    use super::super::test::Rng;
    use super::*;
    use crate::system_functions::u256_advice::reduce_product_with_quotient;
    use num_bigint::BigUint;
    use ruint::aliases::{U256 as HostU256, U512};
    use std::cell::RefCell;
    use std::rc::Rc;
    use zk_ee::oracle::memory_io::host::{
        InProcessMemoryOracle, QuerierMemory, ReadQueryInput, WriteQueryOutput,
    };
    use zk_ee::oracle::query_ids::U256_WIDE_DIV_REM_ADVICE_QUERY_ID;

    /// `U256ModMul` with the general reducer and by-value clones, as it was before the reducer for
    /// residues
    struct LegacyU256ModMul<'a, Adv: ModexpAdvisor> {
        modulus: &'a U256,
        advisor: &'a mut Adv,
    }

    impl<Adv: ModexpAdvisor> ModMul for LegacyU256ModMul<'_, Adv> {
        type Elem = U256;

        fn placeholder(&mut self) -> U256 {
            U256::zero()
        }

        fn clone_elem(&mut self, x: &U256) -> U256 {
            x.clone()
        }

        fn is_zero(&self, x: &U256) -> bool {
            x.is_zero()
        }

        fn square_assign(&mut self, x: &mut U256) {
            let y = x.clone();
            self.mul_assign(x, &y);
        }

        fn mul_assign(&mut self, x: &mut U256, y: &U256) {
            let mut hi = x.clone();
            x.widening_mul_assign_into(&mut hi, y);
            let (q_lo, q_hi) = self.advisor.wide_quotient(x, &hi, self.modulus);
            reduce_product_with_quotient(x, &mut hi, self.modulus, q_lo, q_hi);
        }
    }

    const BN254_R: &str = "30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001";

    fn host(value: &U256) -> HostU256 {
        value.clone().into()
    }

    fn halves(value: U512) -> (HostU256, HostU256) {
        let limbs = value.as_limbs();
        (
            HostU256::from_limbs(limbs[..4].try_into().unwrap()),
            HostU256::from_limbs(limbs[4..].try_into().unwrap()),
        )
    }

    fn reference(base: HostU256, exp: &[u8], modulus: HostU256) -> HostU256 {
        let result = BigUint::from_bytes_be(&base.to_be_bytes::<32>()).modpow(
            &BigUint::from_bytes_be(exp),
            &BigUint::from_bytes_be(&modulus.to_be_bytes::<32>()),
        );
        HostU256::from_be_slice(&result.to_bytes_be())
    }

    /// A query as the oracle receives it: the ID, the operands read through the input word, and
    /// the response words
    type Query = (u32, [HostU256; 3], Vec<u32>);

    /// `base^exp mod modulus` by the legacy or the current multiplication, through the oracle
    /// advisor and an oracle that reads the operands of each query through the querier memory and
    /// answers honestly: the result and the queries in order
    fn exponentiate_over_oracle(
        legacy: bool,
        base: HostU256,
        exp: &[u8],
        modulus: HostU256,
    ) -> (HostU256, Vec<Query>) {
        let queries = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&queries);
        let mut oracle = InProcessMemoryOracle::new(
            move |query_id: u32, input_word: usize, memory: &dyn QuerierMemory| {
                assert_eq!(query_id, U256_WIDE_DIV_REM_ADVICE_QUERY_ID);
                let (lo, hi, m) = <(U256, U256, U256)>::read_input(memory, input_word).unwrap();
                let mut response = vec![];
                naive_wide_quotient(&lo, &hi, &m).write_output(&mut response);
                let operands = [&lo, &hi, &m].map(host);
                log.borrow_mut()
                    .push((query_id, operands, response.clone()));
                response
            },
        );
        let mut advisor = OracleAdvisor { inner: &mut oracle };
        let (base, modulus): (U256, U256) = (base.into(), modulus.into());
        let result = if legacy {
            let mut arithmetic = LegacyU256ModMul {
                modulus: &modulus,
                advisor: &mut advisor,
            };
            exponentiate(&mut arithmetic, &base, exp)
        } else {
            let mut arithmetic = U256ModMul {
                modulus: &modulus,
                advisor: &mut advisor,
            };
            exponentiate(&mut arithmetic, &base, exp)
        };
        let queries = queries.borrow().clone();
        (host(&result), queries)
    }

    /// The exponent walk issues the same queries, with the same operands in memory, in the same
    /// order, and gets the same result as with the legacy multiplication, for moduli of 1 to 4
    /// limbs (even, odd, powers of two, near 2^256, BN254's r) and exponents on both sides of the
    /// window threshold
    #[test]
    fn exponent_walk_queries_match_legacy() {
        let mut rng = Rng(0x0DDB_1A5E_5BAD_5EED);
        let r = HostU256::from_str_radix(BN254_R, 16).unwrap();
        for round in 0..400 {
            let modulus = match round % 8 {
                0 => HostU256::from(1u64) << (2 + rng.next_u64() as usize % 254),
                1 => HostU256::MAX - HostU256::from(rng.next_u64() % 1000),
                2 => r,
                _ => {
                    let mut limbs = [0u64; 4];
                    let significant = 1 + round % 4;
                    for limb in &mut limbs[..significant] {
                        *limb = rng.next_u64();
                    }
                    limbs[significant - 1] |= 1 << 63;
                    limbs[0] = if round % 2 == 0 {
                        limbs[0] & !1
                    } else {
                        limbs[0] | 1
                    };
                    HostU256::from_limbs(limbs)
                }
            };
            let random = HostU256::from_limbs([(); 4].map(|()| rng.next_u64()));
            let base = HostU256::from(2u64) + random % (modulus - HostU256::from(2u64));
            let len = 1 + rng.next_u64() as usize % 40;
            let mut exp = match round % 6 {
                // 63 and 64 bits: the two walks at the window threshold
                0 => {
                    let mut exp = rng.bytes(8);
                    exp[0] = if round % 12 == 0 {
                        exp[0] & 0x7f
                    } else {
                        exp[0] | 0x80
                    };
                    exp
                }
                1 => vec![0xff; len],
                2 if modulus == r => (r - HostU256::from(2u64)).to_be_bytes::<32>().to_vec(),
                3 => rng.bytes(len).into_iter().map(|byte| byte & 0x11).collect(),
                _ => rng.bytes(len),
            };
            if exp[0] == 0 {
                exp[0] = 1;
            }

            let (legacy_result, legacy_queries) =
                exponentiate_over_oracle(true, base, &exp, modulus);
            let (result, queries) = exponentiate_over_oracle(false, base, &exp, modulus);
            assert_eq!(
                result,
                reference(base, &exp, modulus),
                "{base}^{exp:02x?} mod {modulus}"
            );
            assert_eq!(result, legacy_result);
            assert_eq!(queries, legacy_queries, "{base}^{exp:02x?} mod {modulus}");
        }
    }

    /// Answers the wide quotients honestly, except for the call `forged_call`, which gets
    /// `forge(quotient, modulus)`
    struct ForgingAdvisor {
        calls: usize,
        forged_call: usize,
        forge: fn(U512, HostU256) -> U512,
        /// Whether the forged answer differs from the right one, once it is given
        forged: Option<bool>,
    }

    impl ForgingAdvisor {
        fn new(forged_call: usize, forge: fn(U512, HostU256) -> U512) -> Self {
            Self {
                calls: 0,
                forged_call,
                forge,
                forged: None,
            }
        }
    }

    impl ModexpAdvisor for ForgingAdvisor {
        fn get_reduction_op_advice<A: Allocator + Clone>(
            &mut self,
            _a: &BigintRepr<A>,
            _m: &BigintRepr<A>,
            _quotient_dst: &mut BigintRepr<A>,
            _remainder_dst: &mut BigintRepr<A>,
        ) {
            panic!("the single-digit path asks for no reduction advice")
        }

        fn wide_quotient(&mut self, lo: &U256, hi: &U256, modulus: &U256) -> (U256, U256) {
            let (q_lo, q_hi) = naive_wide_quotient(lo, hi, modulus);
            let call = self.calls;
            self.calls += 1;
            if call != self.forged_call {
                return (q_lo, q_hi);
            }
            let right = U512::from(host(&q_lo)) + (U512::from(host(&q_hi)) << 256);
            let forged = (self.forge)(right, host(modulus));
            self.forged = Some(forged != right);
            let (forged_lo, forged_hi) = halves(forged);
            (forged_lo.into(), forged_hi.into())
        }
    }

    /// `base^exp mod modulus` by the legacy or the current multiplication with `advisor`: the
    /// result, or the panic message
    fn exponentiate_with(
        legacy: bool,
        advisor: &mut ForgingAdvisor,
        base: HostU256,
        exp: &[u8],
        modulus: HostU256,
    ) -> Result<HostU256, String> {
        let (base, modulus): (U256, U256) = (base.into(), modulus.into());
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = if legacy {
                let mut arithmetic = LegacyU256ModMul {
                    modulus: &modulus,
                    advisor: &mut *advisor,
                };
                exponentiate(&mut arithmetic, &base, exp)
            } else {
                let mut arithmetic = U256ModMul {
                    modulus: &modulus,
                    advisor: &mut *advisor,
                };
                exponentiate(&mut arithmetic, &base, exp)
            };
            host(&result)
        }))
        .map_err(|payload| match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_owned(),
            Err(payload) => *payload.downcast::<String>().expect("a panic message"),
        })
    }

    /// Wrong answers to a wide quotient `q` modulo `m`: its neighbours, a high half that is not
    /// zero, the extremes, and (for `m = 2^k`) `q + 2^(512 - k)`, whose product with `m` is right
    /// modulo 2^512. A forgery can happen to be the right answer (e.g. 0), which the test checks.
    const FORGERIES: [fn(U512, HostU256) -> U512; 10] = [
        |q, _| q.wrapping_add(U512::from(1u64)),
        |q, _| q.wrapping_sub(U512::from(1u64)),
        |q, _| q.wrapping_add(U512::from(1u64) << 256),
        |q, _| q.wrapping_add(U512::from(1u64) << 511),
        |q, _| q | (U512::from(HostU256::MAX) << 256),
        |q, _| {
            q.wrapping_add(U512::from(1u64) << 256)
                .wrapping_sub(U512::from(1u64))
        },
        |_, _| U512::ZERO,
        |_, _| U512::from(HostU256::MAX),
        |_, _| U512::MAX,
        |q, m| {
            if m.is_power_of_two() {
                q.wrapping_add(U512::from(1u64) << (512 - m.trailing_zeros()))
            } else {
                q.wrapping_add(U512::from(2u64))
            }
        },
    ];

    /// A forged quotient at any product of the walk (a table product, a squaring, a window or bit
    /// multiplication, the last product) is rejected by both multiplications at that product, with
    /// a panic of the reducer; with honest answers both give the reference result from the same
    /// number of answers
    #[test]
    fn forged_quotients_are_rejected_where_they_are_answered() {
        let r = HostU256::from_str_radix(BN254_R, 16).unwrap();
        let bn254_base = HostU256::from_str_radix(
            "01c9d6f3b0e2a4d5c8b7a69584736251403f2e1d0c9b8a79685746352413021f",
            16,
        )
        .unwrap();
        let cases = [
            // windowed: 14 table products, then 4 squarings per nibble and a multiplication per
            // non-zero nibble
            (
                bn254_base,
                (r - HostU256::from(2u64)).to_be_bytes::<32>().to_vec(),
                r,
            ),
            // binary (0b11000): squaring, multiplication, 3 squarings
            (bn254_base, vec![0x18], r),
            (
                HostU256::from(3u64),
                vec![0xff; 9],
                HostU256::from(1u64) << 255,
            ),
            // the largest quotients: (m - 1)^2 / m = m - 2
            (
                HostU256::MAX - HostU256::from(1u64),
                vec![0xff; 32],
                HostU256::MAX,
            ),
        ];
        for (base, exp, modulus) in cases {
            let expected = reference(base, &exp, modulus);
            let mut products = None;
            for legacy in [true, false] {
                let mut honest = ForgingAdvisor::new(usize::MAX, |q, _| q);
                let result = exponentiate_with(legacy, &mut honest, base, &exp, modulus);
                assert_eq!(result, Ok(expected));
                assert_eq!(*products.get_or_insert(honest.calls), honest.calls);
            }
            let products = products.unwrap();
            for forged_call in [0, 1, 13, 14, 22, products - 1] {
                if forged_call >= products {
                    continue;
                }
                for forge in FORGERIES {
                    for legacy in [true, false] {
                        let mut advisor = ForgingAdvisor::new(forged_call, forge);
                        let result = exponentiate_with(legacy, &mut advisor, base, &exp, modulus);
                        match advisor.forged {
                            Some(true) => {
                                let message = result.expect_err("a forged quotient is accepted");
                                assert!(message.starts_with("mulmod hint: "), "{message}");
                                assert_eq!(advisor.calls, forged_call + 1);
                            }
                            Some(false) => assert_eq!(result, Ok(expected)),
                            None => panic!("call {forged_call} of {products} is not answered"),
                        }
                    }
                }
            }
        }
    }
}
