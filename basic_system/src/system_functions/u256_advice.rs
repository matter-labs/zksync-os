use u256::U256;
use zk_ee::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_WIDE_DIV_REM_ADVICE_QUERY_ID};
use zk_ee::oracle::IOOracle;
use zk_ee::system::base_system_functions::{
    AddModNonZeroModulusExt, DivNonZeroDivisorExt, DivRemExt, MulModNonZeroModulusExt,
    RemNonZeroDivisorExt, WideDivRemExt,
};

/// Lives in memory (not in a register) so its address can be handed to the oracle
/// as the high half of a 257-bit dividend.
static ONE: U256 = U256::ONE;
static ZERO: U256 = U256::ZERO;

/// Params for U256 div_rem oracle query (pointer-based, like modexp).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct U256DivRemAdviceParamsGeneric<W> {
    pub dividend_ptr: W,
    pub divisor_ptr: W,
}

pub type U256DivRemAdviceParams = U256DivRemAdviceParamsGeneric<u32>;
pub type U256DivRemAdviceParams64 = U256DivRemAdviceParamsGeneric<u64>;

/// Params for U256 wide div_rem oracle query (512-bit dividend, 256-bit divisor).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct U256WideDivRemAdviceParamsGeneric<W> {
    pub dividend_lo_ptr: W,
    pub dividend_hi_ptr: W,
    pub divisor_ptr: W,
}

pub type U256WideDivRemAdviceParams = U256WideDivRemAdviceParamsGeneric<u32>;
pub type U256WideDivRemAdviceParams64 = U256WideDivRemAdviceParamsGeneric<u64>;

/// Verifies a div_rem hint. On success, `dividend` is modified in-place to
/// hold the remainder. Caller must save q_limbs before calling.
#[must_use]
pub fn verify_div_rem_hint(dividend: &mut U256, divisor: &U256, q: &U256) -> bool {
    // q * divisor must fit in 256 bits
    let mut qd = q.clone();
    if qd.mul_low_assign_overflows(divisor) {
        return false;
    }

    // r = dividend - q * divisor (in-place, avoids clone)
    let borrow = dividend.overflowing_sub_assign(&qd);
    if borrow {
        return false;
    }

    is_less_than(dividend, divisor)
}

/// `a < b` as one subtraction on a scratch copy (a full `Ord::cmp` also tests for
/// equality).
#[inline(always)]
fn is_less_than(a: &U256, b: &U256) -> bool {
    let mut scratch = a.clone();
    scratch.overflowing_sub_assign(b)
}

/// Verifies a wide div_rem hint. On success, `dividend_lo` is modified in-place
/// to hold the remainder. `dividend_hi` is also modified.
#[must_use]
pub fn verify_wide_div_rem_hint(
    dividend_lo: &mut U256,
    dividend_hi: &mut U256,
    divisor: &U256,
    q_lo: &U256,
    q_hi: &U256,
) -> bool {
    // Compute q * divisor as 512-bit
    let mut qd_lo = q_lo.clone();
    let mut qd_mid = q_lo.clone();
    qd_lo.widening_mul_assign_into(&mut qd_mid, divisor);

    let mut qd_hi_lo = q_hi.clone();
    let mut qd_hi_hi = q_hi.clone();
    qd_hi_lo.widening_mul_assign_into(&mut qd_hi_hi, divisor);

    // Accumulate into (qd_lo, qd_mid) — the 512-bit product q*d
    let c1 = qd_mid.overflowing_add_assign(&qd_hi_lo);
    if c1 || !qd_hi_hi.is_zero() {
        return false;
    }

    // Compute r = dividend - q*d in-place (512-bit subtraction)
    let borrow_lo = dividend_lo.overflowing_sub_assign(&qd_lo);
    let borrow_mid = dividend_hi.overflowing_sub_assign(&qd_mid);
    let borrow_final = if borrow_lo {
        dividend_hi.overflowing_sub_assign(&U256::one())
    } else {
        false
    };

    // r must be non-negative (no borrow) and fit in 256 bits (r_hi == 0)
    if (borrow_mid | borrow_final) || !dividend_hi.is_zero() {
        return false;
    }

    *dividend_lo < *divisor
}

/// Asks the oracle for `dividend / divisor` (256-bit operands); the answer is the
/// quotient, 4 limbs.
#[inline(always)]
fn query_div_rem_hint<'a, O: IOOracle>(
    dividend: &U256,
    divisor: &U256,
    oracle: &'a mut O,
) -> O::RawIterator<'a> {
    #[cfg(target_pointer_width = "32")]
    {
        let params = U256DivRemAdviceParams {
            dividend_ptr: (dividend as *const U256).addr() as u32,
            divisor_ptr: (divisor as *const U256).addr() as u32,
        };
        oracle
            .raw_query(
                U256_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256DivRemAdviceParams).addr() as u32),
            )
            .expect("div_rem oracle query failed")
    }

    #[cfg(target_pointer_width = "64")]
    {
        let params = U256DivRemAdviceParams64 {
            dividend_ptr: (dividend as *const U256).addr() as u64,
            divisor_ptr: (divisor as *const U256).addr() as u64,
        };
        oracle
            .raw_query(
                U256_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256DivRemAdviceParams64).addr() as u64),
            )
            .expect("div_rem oracle query failed")
    }
}

/// Asks the oracle for `(dividend_hi * 2^256 + dividend_lo) / divisor`; the answer is
/// the quotient, 8 limbs (low half first).
#[inline(always)]
pub(crate) fn query_wide_div_rem_hint<'a, O: IOOracle>(
    dividend_lo: &U256,
    dividend_hi: &U256,
    divisor: &U256,
    oracle: &'a mut O,
) -> O::RawIterator<'a> {
    #[cfg(target_pointer_width = "32")]
    {
        let params = U256WideDivRemAdviceParams {
            dividend_lo_ptr: (dividend_lo as *const U256).addr() as u32,
            dividend_hi_ptr: (dividend_hi as *const U256).addr() as u32,
            divisor_ptr: (divisor as *const U256).addr() as u32,
        };
        oracle
            .raw_query(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256WideDivRemAdviceParams).addr() as u32),
            )
            .expect("wide_div_rem oracle query failed")
    }

    #[cfg(target_pointer_width = "64")]
    {
        let params = U256WideDivRemAdviceParams64 {
            dividend_lo_ptr: (dividend_lo as *const U256).addr() as u64,
            dividend_hi_ptr: (dividend_hi as *const U256).addr() as u64,
            divisor_ptr: (divisor as *const U256).addr() as u64,
        };
        oracle
            .raw_query(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256WideDivRemAdviceParams64).addr() as u64),
            )
            .expect("wide_div_rem oracle query failed")
    }
}

/// Reduces `dividend` to `dividend mod divisor` in place with an advised quotient
/// and returns the verified quotient. The caller has ruled out a zero divisor.
#[inline(always)]
fn reduce_with_advice<O: IOOracle>(dividend: &mut U256, divisor: &U256, oracle: &mut O) -> U256 {
    debug_assert!(!divisor.is_zero());
    let q = read_u256_from_oracle_response(&mut query_div_rem_hint(dividend, divisor, oracle));
    assert!(
        verify_div_rem_hint(dividend, divisor, &q),
        "div_rem hint: wrong quotient"
    );
    q
}

pub struct DivNonZeroDivisorImpl<const USE_ADVICE: bool>;
pub struct RemNonZeroDivisorImpl<const USE_ADVICE: bool>;
pub struct AddModNonZeroModulusImpl<const USE_ADVICE: bool>;

impl<const USE_ADVICE: bool> DivNonZeroDivisorExt for DivNonZeroDivisorImpl<USE_ADVICE> {
    fn execute<O: IOOracle>(dividend: &mut U256, divisor: &mut U256, oracle: &mut O) {
        debug_assert!(!divisor.is_zero());
        if USE_ADVICE {
            let q = reduce_with_advice(dividend, divisor, oracle);
            // SAFETY: the divisor slot is a distinct, aligned, initialized U256
            unsafe { U256::write_into_ptr_unchecked(divisor as *mut U256, &q) };
        } else {
            U256::div_rem(dividend, divisor);
            // SAFETY: the two slots are distinct, aligned, initialized U256s
            unsafe { U256::write_into_ptr_unchecked(divisor as *mut U256, dividend) };
        }
    }
}

impl<const USE_ADVICE: bool> RemNonZeroDivisorExt for RemNonZeroDivisorImpl<USE_ADVICE> {
    fn execute<O: IOOracle>(dividend: &mut U256, divisor: &mut U256, oracle: &mut O) {
        debug_assert!(!divisor.is_zero());
        if USE_ADVICE {
            let _q = reduce_with_advice(dividend, divisor, oracle);
            // SAFETY: the two slots are distinct, aligned, initialized U256s
            unsafe { U256::write_into_ptr_unchecked(divisor as *mut U256, dividend) };
        } else {
            U256::div_rem(dividend, divisor);
        }
    }
}

impl<const USE_ADVICE: bool> AddModNonZeroModulusExt for AddModNonZeroModulusImpl<USE_ADVICE> {
    fn execute<O: IOOracle>(a: &mut U256, b: &U256, modulus: &mut U256, oracle: &mut O) {
        debug_assert!(!modulus.is_zero());
        if USE_ADVICE {
            u256_addmod_nonzero_modulus_with_advice(a, b, modulus, oracle)
        } else {
            let carry = a.overflowing_add_assign(b);
            let mut hi = if carry { U256::one() } else { U256::zero() };
            u256_wide_div_rem_naive(a, &mut hi, modulus)
        }
    }
}

/// `modulus = (a + b) mod modulus`; the caller has ruled out a zero modulus. The sum
/// is formed in `a`. Reduced operands (the common case) need at most one subtraction
/// and no oracle; otherwise the quotient is advised: of the 256-bit sum when the
/// addition did not carry, of the 257-bit sum `2^256 + a` when it did.
#[inline]
fn u256_addmod_nonzero_modulus_with_advice<O: IOOracle>(
    a: &mut U256,
    b: &U256,
    modulus: &mut U256,
    oracle: &mut O,
) {
    debug_assert!(!modulus.is_zero());
    let carry = a.overflowing_add_assign(b);
    // t = sum - modulus (wrapping); a borrow means the low 256 bits of the sum are
    // below the modulus
    let mut t = a.clone();
    let borrow = t.overflowing_sub_assign(modulus);

    let result: &U256 = if !carry {
        if borrow {
            // sum < modulus
            a
        } else if is_less_than(&t, modulus) {
            // modulus <= sum < 2 * modulus
            &t
        } else {
            let _q = reduce_with_advice(a, modulus, oracle);
            a
        }
    } else if borrow && is_less_than(&t, modulus) {
        // 2^256 + a - modulus, exact since a < modulus, and already reduced
        &t
    } else if modulus.is_one() {
        // the only modulus whose 257-bit quotient does not fit in 256 bits
        &ZERO
    } else {
        // 257-bit dividend (a, 1): the quotient fits in 256 bits since modulus >= 2
        let mut it = query_wide_div_rem_hint(a, &ONE, modulus, oracle);
        let q_lo = read_u256_from_oracle_response(&mut it);
        let q_hi = read_u256_from_oracle_response(&mut it);
        assert!(q_hi.is_zero(), "addmod hint: quotient too large");
        // q * modulus = (qd_lo, qd_hi); remainder = (a, 1) - (qd_lo, qd_hi)
        let mut qd_lo = q_lo.clone();
        let mut qd_hi = q_lo;
        qd_lo.widening_mul_assign_into(&mut qd_hi, modulus);
        let borrow_lo = a.overflowing_sub_assign(&qd_lo);
        // the high word of the remainder, 1 - qd_hi - borrow_lo, must be zero
        let high_ok = if borrow_lo {
            qd_hi.is_zero()
        } else {
            qd_hi.is_one()
        };
        assert!(
            high_ok && is_less_than(a, modulus),
            "addmod hint: remainder out of range"
        );
        a
    };
    // SAFETY: the modulus slot is a distinct, aligned, initialized U256
    unsafe { U256::write_into_ptr_unchecked(modulus as *mut U256, result) };
}

pub struct DivRemImpl<const USE_ADVICE: bool>;
pub struct WideDivRemImpl<const USE_ADVICE: bool>;
pub struct MulModNonZeroModulusImpl<const USE_ADVICE: bool>;

impl<const USE_ADVICE: bool> MulModNonZeroModulusExt for MulModNonZeroModulusImpl<USE_ADVICE> {
    fn execute<O: IOOracle>(a: &mut U256, b: &U256, modulus: &mut U256, oracle: &mut O) {
        debug_assert!(!modulus.is_zero());
        if USE_ADVICE {
            u256_mulmod_nonzero_modulus_with_advice(a, b, modulus, oracle)
        } else {
            // the product lands in (a, hi)
            let mut hi = a.clone();
            a.widening_mul_assign_into(&mut hi, b);
            u256_wide_div_rem_naive(a, &mut hi, modulus)
        }
    }
}

impl<const USE_ADVICE: bool> DivRemExt for DivRemImpl<USE_ADVICE> {
    fn execute<O: IOOracle>(
        dividend_or_quotient: &mut U256,
        divisor_or_remainder: &mut U256,
        oracle: &mut O,
    ) {
        if USE_ADVICE {
            u256_div_rem_with_advice(dividend_or_quotient, divisor_or_remainder, oracle)
        } else {
            U256::div_rem(dividend_or_quotient, divisor_or_remainder)
        }
    }
}

impl<const USE_ADVICE: bool> WideDivRemExt for WideDivRemImpl<USE_ADVICE> {
    fn execute<O: IOOracle>(
        dividend_lo: &mut U256,
        dividend_hi: &mut U256,
        divisor: &mut U256,
        oracle: &mut O,
    ) {
        if USE_ADVICE {
            u256_wide_div_rem_with_advice(dividend_lo, dividend_hi, divisor, oracle)
        } else {
            u256_wide_div_rem_naive(dividend_lo, dividend_hi, divisor)
        }
    }
}

/// Reads one U256 of the hint straight into an aligned slot: the oracle streams the
/// little-endian words of the value, which is the memory layout of the limbs, so
/// each word is stored where it belongs instead of being assembled in registers
/// and copied.
#[inline(always)]
pub(crate) fn read_u256_from_oracle_response(
    it: &mut impl ExactSizeIterator<Item = usize>,
) -> U256 {
    const WORDS: usize = core::mem::size_of::<U256>() / core::mem::size_of::<usize>();
    let mut out = core::mem::MaybeUninit::<U256>::uninit();
    let dst = out.as_mut_ptr().cast::<usize>();
    for i in 0..WORDS {
        let word = it.next().expect("u256 hint word");
        // SAFETY: `dst` points to the 32 bytes of `out`, `i` stays inside them
        unsafe { dst.add(i).write(word) };
    }
    // SAFETY: all words of `out` were written
    unsafe { out.assume_init() }
}

#[inline]
pub fn u256_div_rem_with_advice<O: IOOracle>(
    dividend_or_quotient: &mut U256,
    divisor_or_remainder: &mut U256,
    oracle: &mut O,
) {
    assert!(!divisor_or_remainder.is_zero());
    let q = reduce_with_advice(dividend_or_quotient, divisor_or_remainder, oracle);
    // SAFETY: the two slots are distinct, aligned, initialized U256s
    unsafe {
        U256::write_into_ptr_unchecked(divisor_or_remainder as *mut U256, dividend_or_quotient);
        U256::write_into_ptr_unchecked(dividend_or_quotient as *mut U256, &q);
    }
}

fn u256_wide_div_rem_naive(dividend_lo: &mut U256, dividend_hi: &mut U256, divisor: &mut U256) {
    assert!(!divisor.is_zero());
    let mut dividend = [0u64; 8];
    dividend[..4].copy_from_slice(dividend_lo.as_limbs());
    dividend[4..].copy_from_slice(dividend_hi.as_limbs());
    let mut d = *divisor.as_limbs();
    ruint::algorithms::div(&mut dividend, &mut d);
    *divisor = U256::from_limbs(d);
}

#[inline]
fn u256_wide_div_rem_with_advice<O: IOOracle>(
    dividend_lo: &mut U256,
    dividend_hi: &mut U256,
    divisor: &mut U256,
    oracle: &mut O,
) {
    assert!(!divisor.is_zero());

    let mut it = query_wide_div_rem_hint(dividend_lo, dividend_hi, divisor, oracle);

    let q_lo = read_u256_from_oracle_response(&mut it);
    let q_hi = read_u256_from_oracle_response(&mut it);

    // verify modifies dividend_lo in-place to hold the remainder
    assert!(verify_wide_div_rem_hint(
        dividend_lo,
        dividend_hi,
        divisor,
        &q_lo,
        &q_hi
    ));

    core::mem::swap(dividend_lo, divisor);
}

/// `modulus = a * b mod modulus` with the quotient of the 512-bit product advised by
/// the oracle. The caller has already ruled out a zero modulus. Works in place on
/// the three operands: the product is formed in `a` (low half) and a scratch (high
/// half), the quotient check runs on the modulus, and the remainder is copied into
/// the modulus slot, so no operand is ever swapped.
#[inline]
fn u256_mulmod_nonzero_modulus_with_advice<O: IOOracle>(
    a: &mut U256,
    b: &U256,
    modulus: &mut U256,
    oracle: &mut O,
) {
    debug_assert!(!modulus.is_zero());

    // product = (product_lo, product_hi) = a * b; `a` becomes the low half
    let mut product_hi = a.clone();
    let product_lo = a;
    product_lo.widening_mul_assign_into(&mut product_hi, b);

    let mut it = query_wide_div_rem_hint(product_lo, &product_hi, modulus, oracle);
    let q_lo = read_u256_from_oracle_response(&mut it);
    let q_hi = read_u256_from_oracle_response(&mut it);
    reduce_product_with_quotient(product_lo, &mut product_hi, modulus, q_lo, q_hi);

    // SAFETY: the modulus slot is a distinct, aligned, initialized U256
    unsafe { U256::write_into_ptr_unchecked(modulus as *mut U256, product_lo) };
}

/// Reduces the 512-bit product `(product_lo, product_hi)` modulo `modulus` with its advised
/// quotient `(q_lo, q_hi)`: the remainder `product - q * modulus` is formed in `product_lo`
/// and checked to be in `[0, modulus)`. Panics on a wrong quotient. `product_hi` is
/// consumed as scratch.
#[inline]
pub(crate) fn reduce_product_with_quotient(
    product_lo: &mut U256,
    product_hi: &mut U256,
    modulus: &U256,
    mut q_lo: U256,
    mut q_hi: U256,
) {
    // q * modulus as 512 bits: q_lo * m = (qd_lo, qd_mid), q_hi * m must fit in 256 bits
    let qd_lo = &mut q_lo;
    let mut qd_mid = qd_lo.clone();
    qd_lo.widening_mul_assign_into(&mut qd_mid, modulus);
    let qd_hi_lo = &mut q_hi;
    let mut qd_hi_hi = qd_hi_lo.clone();
    qd_hi_lo.widening_mul_assign_into(&mut qd_hi_hi, modulus);
    let carry = qd_mid.overflowing_add_assign(qd_hi_lo);
    assert!(
        !carry && qd_hi_hi.is_zero(),
        "mulmod hint: quotient too large"
    );

    // remainder = product - q * modulus, formed in the low half; must not borrow and
    // must fit in 256 bits
    let borrow_lo = product_lo.overflowing_sub_assign(qd_lo);
    let borrow_mid = product_hi.overflowing_sub_assign_with_borrow_propagation(&qd_mid, borrow_lo);
    assert!(
        !borrow_mid && product_hi.is_zero(),
        "mulmod hint: remainder out of range"
    );

    // remainder < modulus: a subtraction on scratch must borrow
    let mut check = product_lo.clone();
    let borrow = check.overflowing_sub_assign(modulus);
    assert!(borrow, "mulmod hint: remainder not reduced");
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruint::aliases::{U256 as HostU256, U512};
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::system::base_system_functions::{
        AddModNonZeroModulusExt, DivNonZeroDivisorExt, MulModNonZeroModulusExt,
        RemNonZeroDivisorExt,
    };
    use zk_ee::system::errors::internal::InternalError;

    /// The paths without advice never query the oracle.
    struct NoOracle;

    impl IOOracle for NoOracle {
        type RawIterator<'a> = core::iter::Empty<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            panic!("the path without advice must not query the oracle")
        }
    }

    /// Answers the two division queries the way the host does: reads the operands
    /// through the pointers of the params struct and returns the quotient limbs.
    struct HostOracle {
        queries: usize,
    }

    fn read_u256(ptr: u64) -> HostU256 {
        // SAFETY: the tests pass addresses of live U256 values
        let value: &U256 = unsafe { &*(ptr as usize as *const U256) };
        host(value)
    }

    fn host(value: &U256) -> HostU256 {
        value.clone().into()
    }

    impl IOOracle for HostOracle {
        type RawIterator<'a> = std::vec::IntoIter<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            query_type: u32,
            input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            self.queries += 1;
            let arg_ptr = input.iter().next().expect("params pointer");
            let quotient: Vec<u64> = if query_type == U256_DIV_REM_ADVICE_QUERY_ID {
                // SAFETY: the tests pass the address of a live params struct
                let params: U256DivRemAdviceParams64 =
                    unsafe { *(arg_ptr as *const U256DivRemAdviceParams64) };
                let q: HostU256 = read_u256(params.dividend_ptr) / read_u256(params.divisor_ptr);
                q.as_limbs().to_vec()
            } else if query_type == U256_WIDE_DIV_REM_ADVICE_QUERY_ID {
                // SAFETY: the tests pass the address of a live params struct
                let params: U256WideDivRemAdviceParams64 =
                    unsafe { *(arg_ptr as *const U256WideDivRemAdviceParams64) };
                let dividend = U512::from(read_u256(params.dividend_lo_ptr))
                    + (U512::from(read_u256(params.dividend_hi_ptr)) << 256);
                let q: U512 = dividend / U512::from(read_u256(params.divisor_ptr));
                q.as_limbs().to_vec()
            } else {
                panic!("unexpected query {query_type:#x}")
            };
            Ok(quotient
                .into_iter()
                .map(|limb| limb as usize)
                .collect::<Vec<_>>()
                .into_iter())
        }
    }

    fn samples() -> [HostU256; 12] {
        [
            HostU256::ZERO,
            HostU256::from(1u64),
            HostU256::from(2u64),
            HostU256::from(7u64),
            HostU256::from(1u64) << 128,
            HostU256::from_limbs([u64::MAX, u64::MAX, 0, 0]),
            HostU256::from_limbs([0, 0, 0, 0xdead_beef_cafe_babe]),
            HostU256::from_limbs([
                0x0123_4567_89ab_cdef,
                0xfedc_ba98_7654_3210,
                0x0011_2233_4455_6677,
                0x7f00_0000_0000_0000,
            ]),
            HostU256::from_limbs([3, 0, 0, 1u64 << 63]),
            HostU256::MAX >> 1,
            HostU256::MAX - HostU256::from(1u64),
            HostU256::MAX,
        ]
    }

    fn reference_mulmod(a: HostU256, b: HostU256, m: HostU256) -> HostU256 {
        let r = a.widening_mul(b) % U512::from(m);
        HostU256::from_limbs(r.as_limbs()[..4].try_into().unwrap())
    }

    fn reference_addmod(a: HostU256, b: HostU256, m: HostU256) -> HostU256 {
        let r = (U512::from(a) + U512::from(b)) % U512::from(m);
        HostU256::from_limbs(r.as_limbs()[..4].try_into().unwrap())
    }

    fn check_all<const USE_ADVICE: bool, O: IOOracle>(oracle: &mut O) {
        for a in samples() {
            for b in samples() {
                if !b.is_zero() {
                    let mut a_u: U256 = a.into();
                    let mut b_u: U256 = b.into();
                    DivNonZeroDivisorImpl::<USE_ADVICE>::execute(&mut a_u, &mut b_u, oracle);
                    assert_eq!(host(&b_u), a / b, "{a} / {b}");

                    let mut a_u: U256 = a.into();
                    let mut b_u: U256 = b.into();
                    RemNonZeroDivisorImpl::<USE_ADVICE>::execute(&mut a_u, &mut b_u, oracle);
                    assert_eq!(host(&b_u), a % b, "{a} % {b}");
                }
                for m in samples() {
                    if m.is_zero() {
                        continue;
                    }
                    let mut a_u: U256 = a.into();
                    let b_u: U256 = b.into();
                    let mut m_u: U256 = m.into();
                    MulModNonZeroModulusImpl::<USE_ADVICE>::execute(
                        &mut a_u, &b_u, &mut m_u, oracle,
                    );
                    assert_eq!(host(&m_u), reference_mulmod(a, b, m), "{a} * {b} mod {m}");

                    let mut a_u: U256 = a.into();
                    let mut m_u: U256 = m.into();
                    AddModNonZeroModulusImpl::<USE_ADVICE>::execute(
                        &mut a_u, &b_u, &mut m_u, oracle,
                    );
                    assert_eq!(host(&m_u), reference_addmod(a, b, m), "{a} + {b} mod {m}");
                }
            }
        }
    }

    #[test]
    fn arithmetic_without_advice_matches_reference() {
        check_all::<false, _>(&mut NoOracle);
    }

    #[test]
    fn arithmetic_with_advice_matches_reference() {
        let mut oracle = HostOracle { queries: 0 };
        check_all::<true, _>(&mut oracle);
        assert!(oracle.queries > 0);
    }

    #[test]
    fn reduced_addmod_operands_need_no_advice() {
        // a, b < m: at most one subtraction, never a query
        let m = HostU256::from_limbs([0, 0, 0, 1u64 << 62]);
        for a in samples() {
            for b in samples() {
                if a >= m || b >= m {
                    continue;
                }
                let mut a_u: U256 = a.into();
                let b_u: U256 = b.into();
                let mut m_u: U256 = m.into();
                AddModNonZeroModulusImpl::<true>::execute(&mut a_u, &b_u, &mut m_u, &mut NoOracle);
                assert_eq!(host(&m_u), reference_addmod(a, b, m));
            }
        }
    }

    #[test]
    #[should_panic(expected = "div_rem hint: wrong quotient")]
    fn wrong_quotient_is_rejected() {
        struct WrongOracle;
        impl IOOracle for WrongOracle {
            type RawIterator<'a> = std::vec::IntoIter<usize>;
            fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
                &'a mut self,
                _query_type: u32,
                _input: &I,
            ) -> Result<Self::RawIterator<'a>, InternalError> {
                Ok(vec![3usize, 0, 0, 0].into_iter())
            }
        }
        let mut a: U256 = HostU256::from(35u64).into();
        let mut b: U256 = HostU256::from(6u64).into();
        DivNonZeroDivisorImpl::<true>::execute(&mut a, &mut b, &mut WrongOracle);
    }
}
